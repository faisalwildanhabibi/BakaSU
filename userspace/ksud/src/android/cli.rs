use std::path::PathBuf;

use android_logger::Config;
use anyhow::{Context, Ok, Result};
use clap::Parser;
use log::{LevelFilter, error, info};

use crate::{
    android::{
        debug, dynamic_manager, feature, init_event, ksucalls,
        module::{self, module_config, regenerate_preinit},
        profile, sepolicy, su, sulog, susfs, uapi, umount_config, utils,
    },
    anykernel3::{self, Slot},
    apk_sign, assets,
    boot_patch::{BootPatchArgs, BootRestoreArgs},
    defs,
    lkm_image::BootPatchV2Args,
};

/// KernelSU userspace cli
#[derive(Parser, Debug)]
#[command(author, version = defs::FULL_VERSION, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Flash an AnyKernel3 ZIP
    #[command(name = "anykernel3")]
    AnyKernel3 {
        /// AnyKernel3 ZIP file path
        zip: PathBuf,

        /// Select A/B slot for flashing
        #[arg(long, value_enum)]
        slot: Option<Slot>,
    },

    /// Manage KernelSU modules
    Module {
        #[command(subcommand)]
        command: Module,
    },

    /// Trigger `post-fs-data` event
    PostFsData,

    /// Trigger `service` event
    Services,

    /// Run sulog reader daemon. Not for user. Use `ksud debug sulogd` to launch daemon.
    #[command(hide = true)]
    Sulogd,

    /// Trigger `boot-complete` event
    BootCompleted,

    /// Load kernelsu.ko and execute late-load stage scripts
    LateLoad {
        /// Use adb root to execute late-load for jailbreaking by Magica
        #[arg(long, default_missing_value = "5555", num_args = 0..=1)]
        magica: Option<u16>,

        /// Pass allow_shell=1 when loading kernelsu.ko
        #[arg(long)]
        allow_shell: bool,

        /// Restore adb properties after magica late-load
        #[arg(long)]
        post_magica: bool,

        /// Specify kernel KMI version instead of auto-detection
        #[arg(long)]
        kmi: Option<String>,

        /// manager package name
        #[arg(long, default_value_t = String::from(defs::DEFAULT_PACKAGE_NAME))]
        package_name: String,
    },

    /// Manage auto apply user custom umount configs
    UmountConfig {
        #[command(subcommand)]
        command: UmountConfigOp,
    },

    /// Emulate system reboot
    SoftReboot,

    /// Load a kernel module with kallsyms access
    Insmod {
        /// kernel module path
        module: PathBuf,
        /// module load parameters (e.g. key=val key2=val2)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        params: Vec<String>,
    },

    /// Install KernelSU userspace component to system
    Install {
        #[arg(long, default_value = None)]
        libadbroot: Option<PathBuf>,

        #[arg(long, default_value = None)]
        data_path: Option<PathBuf>,
    },

    /// Unload KernelSU kernel module (LKM Only)
    Unload,

    /// Uninstall KernelSU modules and itself(LKM Only)
    Uninstall {
        #[arg(long, default_value_t = String::from("com.resukisu.resukisu"))]
        package_name: String,
    },

    /// SELinux policy Patch tool
    Sepolicy {
        #[command(subcommand)]
        command: Sepolicy,
    },

    /// Manage App Profiles
    Profile {
        #[command(subcommand)]
        command: Profile,
    },

    /// Manage kernel features
    Feature {
        #[command(subcommand)]
        command: Feature,
    },

    /// Patch boot or init_boot images to apply KernelSU
    BootPatch(BootPatchArgs),

    /// Restore boot or init_boot images patched by KernelSU
    BootRestore(BootRestoreArgs),

    /// Patch KernelSU into a boot image
    ///
    /// Always operates on a boot image; never selects init_boot or vendor_boot.
    BootPatchV2(BootPatchV2Args),

    /// Show boot information
    BootInfo {
        #[command(subcommand)]
        command: BootInfo,
    },

    /// For developers
    Debug {
        #[command(subcommand)]
        command: Debug,
    },
    /// Kernel interface
    Kernel {
        #[command(subcommand)]
        command: Kernel,
    },

    /// Resetprop - Magisk-compatible system property tool
    Resetprop(crate::android::resetprop::Args),

    /// Manage susfs component
    Susfs(susfs::cli::SusfsArgs),

    /// Manage initrc injection
    Initrc {
        #[command(subcommand)]
        command: Initrc,
    },

    /// Manage thermal profile, charge limit and bypass charging
    Tuning {
        #[command(subcommand)]
        command: TuningCmd,
    },

    /// Manage built-in Zygisk
    Zygisk {
        #[command(subcommand)]
        command: ZygiskCommand,
    },

    /// Manage built-in HMA OSS and UID isolation
    Hma {
        #[command(subcommand)]
        command: HmaCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum TuningCmd {
    /// Show current thermal / bypass charging status
    Status,

    /// List supported thermal profiles
    List,

    /// Apply a thermal profile and remember it for the next boot
    Thermal {
        /// profile index (see `ksud tuning list`)
        index: i32,
    },

    /// Per-app thermal: map an app UID to a thermal profile
    AppThermal {
        /// app UID
        uid: u32,
        /// profile index (see `ksud tuning list`)
        index: i32,
    },

    /// Per-app thermal: remove an app mapping
    AppThermalRemove {
        /// app UID
        uid: u32,
    },

    /// Per-app thermal: clear every mapping
    AppThermalClear,

    /// Per-app thermal: list mappings as uid=index
    AppThermalList,

    /// Run the thermal / charge-limit watcher (internal daemon)
    #[command(hide = true)]
    Watch,

    /// Charging: set the battery percentage at which charging stops (70-100)
    ChargeLimit { percent: i32 },

    /// Charging: enable (1) or disable (0) the percentage-based charge limit
    ChargeLimitToggle { value: u8 },

    /// Charging: toggle sysfs writable mode (1=0644, 0=0444)
    ChargeWritable { value: u8 },

    /// Charging: set the constant charge current in microamps
    ChargeCurrent { ua: u64 },

    /// Enable (1) or disable (0) bypass charging and remember it for the next boot
    Bypass {
        /// 1 to enable, 0 to disable
        value: u8,
    },

    /// Toggle DVFS headroom (0 = enable, 1 = disable)
    SchedSkipHeadroom { value: u8 },

    /// Toggle PELT multiplier (0 = disable, 1 = enable)
    SchedPeltMultiplier { value: u8 },

    /// Toggle DVFS headroom scaling (0 = off, 1 = on)
    SchedHrScaling { value: u8 },

    /// Little cores scaling (-50..100)
    SchedBoostHrLp { value: i32 },

    /// Perf cores scaling (-50..100)
    SchedBoostHrBig { value: i32 },

    /// Prime core scaling (-50..30)
    SchedBoostHrPrime { value: i32 },

    /// Set a cgroup tunable
    Cgroup {
        /// background | top-app | foreground
        name: String,
        /// uclamp_min | uclamp_max | shares
        key: String,
        value: i32,
    },

    /// Enable (1) or disable (0) restoring tuning values across reboots
    Persist { value: u8 },

    /// Set TCP Congestion Algorithm
    TcpCongestion { value: String },

    /// Set Queueing Discipline
    Qdisc { value: String },

    /// Memory: Set swappiness (0-100)
    Swappiness { value: i32 },
    /// Memory: Set vfs_cache_pressure
    VfsCache { value: i32 },
    /// Memory: Set kcompressd
    Kcompressd { value: i32 },

    /// Memory: Enable/disable MGLRU (1/0)
    Mglru { value: u8 },
    /// Memory: Set MGLRU min_ttl_ms
    MglruTtl { value: i32 },

    /// zRAM: set size in GB (2, 3, 4, 6, 8)
    ZramSize { gb: u64 },

    /// zRAM: set compression algorithm (lzo, lz4, lz4hc, zstd, deflate, 842)
    ZramAlgorithm { algorithm: String },

    /// Reapply the persisted tuning settings
    Apply,
}

#[derive(clap::Subcommand, Debug)]
pub enum ZygiskCommand {
    /// Enable built-in Zygisk and start it now
    On,
    /// Disable built-in Zygisk
    Off,
    /// Check built-in Zygisk status
    Status,
    /// Report the active Zygisk implementation (running, reboot, disabled or none)
    Info,
    /// Switch to the built-in Zygisk or to an installed provider module, live
    Switch {
        /// `builtin` or the id of an installed Zygisk module
        #[arg(default_value = "builtin")]
        target: String,
    },
    /// List the monitor and daemon processes of an implementation as JSON
    Processes {
        /// `builtin` or the id of an installed Zygisk module
        #[arg(default_value = "builtin")]
        target: String,
    },
    /// List the Zygisk modules the built-in is serving, with their processes, as JSON
    Modules,
    /// Reload built-in Zygisk
    Rearm,
    /// Synchronize all isolated/denylist UIDs to NoMount and enable block_isolated
    NomountSync,
    /// Read or change BuiltZygisk options (`config.prop`)
    Config {
        #[command(subcommand)]
        command: ZygiskConfigCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ZygiskConfigCommand {
    /// Print all options as JSON
    Get,
    /// Set one option, e.g. `ksud zygisk config set custom_linker 1`
    Set { key: String, value: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaCommand {
    /// Enable built-in HMA OSS + UID
    On,
    /// Disable built-in HMA OSS + UID
    Off,
    /// Report built-in HMA OSS + UID status as JSON
    Status,
    /// Synchronize UID scope rules for active target apps
    Sync,
    /// Read or update HMA global config (`config.json`)
    Config {
        #[command(subcommand)]
        command: HmaConfigCommand,
    },
    /// Manage HMA hide templates
    Template {
        #[command(subcommand)]
        command: HmaTemplateCommand,
    },
    /// Manage per-app HMA scope rules
    Scope {
        #[command(subcommand)]
        command: HmaScopeCommand,
    },
    /// Manage TargetedHide-style per-app and global path hiding
    Paths {
        #[command(subcommand)]
        command: HmaPathsCommand,
    },
    /// Inspect built-in official app presets and settings presets
    Preset {
        #[command(subcommand)]
        command: HmaPresetCommand,
    },
    /// Manage HMA settings templates (Accessibility, Input Method, Dev Options, custom)
    SettingsTemplate {
        #[command(subcommand)]
        command: HmaSettingsTemplateCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaPathsCommand {
    /// Print HMA path hiding configuration as JSON
    Get,
    /// Toggle automatic hiding of hidden apps' install and data directories
    SetAuto { enabled: String },
    /// Set global hidden paths (comma-separated, supports `!` exclusions)
    SetGlobal {
        #[arg(default_value = "")]
        paths: String,
    },
    /// Set per-app hidden paths (comma-separated, supports `!` exclusions)
    SetApp {
        package: String,
        #[arg(default_value = "")]
        paths: String,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaConfigCommand {
    /// Print full HMA config.json
    Get,
    /// Set one global config option, e.g. `ksud hma config set forceMountData true`
    Set { key: String, value: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaTemplateCommand {
    /// List all templates and presets as JSON
    List,
    /// Get details of a template or preset as JSON
    Get { name: String },
    /// Create or update a template
    Set {
        name: String,
        #[arg(long)]
        whitelist: bool,
        #[arg(default_value = "")]
        apps: String,
    },
    /// Delete a template
    Delete { name: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaScopeCommand {
    /// List all configured app scopes as JSON
    List,
    /// Get an app's scope config as JSON
    Get { package: String },
    /// Set or update an app's scope config as JSON
    Set {
        package: String,
        config_json: String,
    },
    /// Remove an app from scope and add to ignoredPackagesForPresets
    Remove { package: String },
    /// Evaluate and debug effective hidden UIDs, packages, and settings for an app
    Check { package: String },
    /// Clear all apps from scope
    Clear,
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaPresetCommand {
    /// List all official app presets and settings presets with summary
    List,
    /// Get details of a built-in preset
    Get { name: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum HmaSettingsTemplateCommand {
    /// List all settings templates and settings presets
    List,
    /// Get a settings template as JSON
    Get { name: String },
    /// Create or update a settings template
    Set { name: String, json: String },
    /// Delete a custom settings template
    Delete { name: String },
}

#[derive(clap::Subcommand, Debug)]
enum UmountConfigOp {
    /// Add an new umount config to configuration file
    Add {
        /// mount point path
        mnt: String,
        /// umount flags (default: 0, MNT_DETACH: 2)
        #[arg(short, long, default_value = "0")]
        flags: u32,
    },
    /// Delete an umount config from configuration file
    Del {
        /// mount point path
        mnt: String,
    },
    /// Clear all auto apply umount config from configuration file
    Clear,
    /// List all configured auto apply umount configuration
    List,
}

#[derive(clap::Subcommand, Debug)]
enum BootInfo {
    /// show current kmi version
    CurrentKmi,

    /// show supported kmi versions
    SupportedKmis,

    /// check if device is A/B capable
    IsAbDevice,

    /// show auto-selected boot partition name
    DefaultPartition,

    /// list available partitions for current or OTA toggled slot
    AvailablePartitions,

    /// show slot suffix for current or OTA toggled slot
    SlotSuffix {
        /// toggle to another slot
        #[arg(short = 'u', long, default_value = "false")]
        ota: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
enum Debug {
    /// Set the manager app, kernel CONFIG_KSU_DEBUG should be enabled.
    SetManager {
        /// manager package name
        #[arg(default_value_t = String::from(defs::DEFAULT_PACKAGE_NAME))]
        apk: String,
    },

    /// Get apk size and hash
    GetSign {
        /// apk path
        apk: String,
    },

    /// Root Shell
    Su {
        /// switch to gloabl mount namespace
        #[arg(short, long, default_value = "false")]
        global_mnt: bool,
    },

    /// Get kernel version
    Version,

    /// For testing
    Test,

    /// Extract an embedded binary to a specified path
    ExtractBinary {
        /// binary name (e.g. busybox, resetprop, bootctl)
        name: String,
        /// destination file path
        path: PathBuf,
    },

    /// Process mark management
    Mark {
        #[command(subcommand)]
        command: MarkCommand,
    },

    /// Launch sulogd daemon manually
    Sulogd,

    /// Get kernel info
    Info,

    /// Print default package name
    Package,
}

#[derive(clap::Subcommand, Debug)]
enum MarkCommand {
    /// Get mark status for a process (or all)
    Get {
        /// target pid (0 for total count)
        #[arg(default_value = "0")]
        pid: i32,
    },

    /// Mark a process
    Mark {
        /// target pid (0 for all processes)
        #[arg(default_value = "0")]
        pid: i32,
    },

    /// Unmark a process
    Unmark {
        /// target pid (0 for all processes)
        #[arg(default_value = "0")]
        pid: i32,
    },

    /// Refresh mark for all running processes
    Refresh,
}

#[derive(clap::Subcommand, Debug)]
enum Sepolicy {
    /// Patch sepolicy
    Patch {
        /// sepolicy statements
        sepolicy: String,
    },

    /// Apply sepolicy from file
    Apply {
        /// sepolicy file path
        file: String,
    },

    /// Check if sepolicy statement is supported/valid
    Check {
        /// sepolicy statements
        sepolicy: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum Module {
    /// Install module <ZIP>
    Install {
        /// module zip file path
        zip: String,
    },

    /// Undo module uninstall mark <id>
    UndoUninstall {
        /// module id
        id: String,
    },

    /// Uninstall module <id>
    Uninstall {
        /// module id
        id: String,
    },

    /// enable module <id>
    Enable {
        /// module id
        id: String,
    },

    /// disable module <id>
    Disable {
        // module id
        id: String,
    },

    /// run action for module <id>
    Action {
        // module id
        id: String,
    },

    /// list all modules
    List,

    /// preinit actions run before SELinux policy is loaded
    Preinit {
        #[command(subcommand)]
        command: PreinitCmd,
    },

    /// manage module configuration
    Config {
        /// target internal module name (resolved as internal.<name>)
        #[arg(long)]
        internal: Option<String>,
        #[command(subcommand)]
        command: ModuleConfigCmd,
    },
}

#[derive(clap::Subcommand, Debug)]
enum PreinitCmd {
    /// show per-module preinit state for the next boot
    Status,
    /// re-enable preinit after it was disabled by repeated failed boots
    Reset,
}

#[derive(clap::Subcommand, Debug)]
enum ModuleConfigCmd {
    /// Get a config value
    Get {
        /// config key
        key: String,
    },

    /// Set a config value
    Set {
        /// config key
        key: String,
        /// config value (omit to read from stdin)
        value: Option<String>,
        /// read value from stdin (default if value not provided)
        #[arg(long)]
        stdin: bool,
        /// use temporary config (cleared on reboot)
        #[arg(short, long)]
        temp: bool,
    },

    /// List all config entries
    List,

    /// Delete a config entry
    Delete {
        /// config key
        key: String,
        /// delete from temporary config
        #[arg(short, long)]
        temp: bool,
    },

    /// Clear all config entries
    Clear {
        /// clear temporary config
        #[arg(short, long)]
        temp: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
enum Profile {
    /// get root profile's selinux policy of <package-name>
    GetSepolicy {
        /// package name
        package: String,
    },

    /// set root profile's selinux policy of <package-name> to <profile>
    SetSepolicy {
        /// package name
        package: String,
        /// policy statements
        policy: String,
    },

    /// get template of <id>
    GetTemplate {
        /// template id
        id: String,
    },

    /// set template of <id> to <template string>
    SetTemplate {
        /// template id
        id: String,
        /// template string
        template: String,
    },

    /// delete template of <id>
    DeleteTemplate {
        /// template id
        id: String,
    },

    /// list all templates
    ListTemplates {
        /// print template names instead of ids
        #[arg(short, long)]
        name: bool,

        /// locale used to resolve localized template names (for example, zh_CN)
        #[arg(long, requires = "name")]
        locale: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
enum Feature {
    /// Get feature value and support status
    Get {
        /// Feature ID or name (su_compat, kernel_umount, sulog, adb_root, selinux_hide, shell_su_adb_only)
        id: String,
        /// Read from config file
        #[arg(long, default_value_t = false)]
        config: bool,
    },

    /// Set feature value
    Set {
        /// Feature ID or name
        id: String,
        /// Feature value (0=disable, 1=enable)
        value: u64,
    },

    /// List all available features
    List,

    /// Check feature status (supported/unsupported/managed/forced)
    Check {
        /// Feature ID or name (su_compat, kernel_umount, sulog, adb_root, selinux_hide, shell_su_adb_only)
        id: String,
    },

    /// Load configuration from file and apply to kernel
    Load,

    /// Save current kernel feature states to file
    Save,
}

#[derive(clap::Subcommand, Debug)]
enum Kernel {
    /// Nuke ext4 sysfs
    NukeExt4Sysfs {
        /// mount point
        mnt: String,
    },
    /// Manage umount list
    Umount {
        #[command(subcommand)]
        command: UmountOp,
    },
    /// Manage dynamic manager
    DynamicManager {
        #[command(subcommand)]
        command: DynamicManagerOp,
    },
    /// Notify that module is mounted
    NotifyModuleMounted,
}

#[derive(clap::Subcommand, Debug)]
enum DynamicManagerOp {
    /// Get the signature of the current dynamic manager (size+hash)
    Get {
        #[arg(long)]
        internal: Option<bool>,
    },
    /// Set the signature of the dynamic manager
    Set {
        /// the signature size
        size: u32,
        /// the signature hash
        #[arg(value_parser = dynamic_manager::parse_hash)]
        hash: [u8; 64],
    },
    /// Set the signature of the dynamic manager for apk
    SetApk {
        /// the apk path
        apk: String,
    },
    /// Clear the dynamic manager
    Clear,
}

#[derive(clap::Subcommand, Debug)]
enum UmountOp {
    /// Add mount point to umount list
    Add {
        /// mount point path
        mnt: String,
        /// umount flags (default: 0, MNT_DETACH: 2)
        #[arg(short, long, default_value = "0")]
        flags: u32,
    },
    /// Delete mount point from umount list
    Del {
        /// mount point path
        mnt: String,
    },
    /// Wipe all entries from umount list
    Wipe,
    /// List all entries from umount list
    List,
}

#[derive(clap::Subcommand, Debug)]
enum Initrc {
    /// Regenerate preinit rc file and preinit dirs
    Refresh,
}

#[allow(clippy::similar_names)]
pub fn run() -> Result<()> {
    android_logger::init_once(
        Config::default()
            .with_max_level(crate::debug_select!(LevelFilter::Trace, LevelFilter::Info))
            .with_tag("KernelSU"),
    );

    ksucalls::setup_sigsys_handler();

    // the kernel executes su with argv[0] = "su" and replace it with us
    let arg0 = std::env::args().next().unwrap_or_default();
    if arg0 == "su" || arg0.ends_with("/su") {
        return su::root_shell();
    }

    if arg0.ends_with("resetprop") {
        let all_args: Vec<String> = std::env::args().collect();
        return crate::android::resetprop::run_from_args(&all_args);
    }

    if arg0.ends_with("ksu_susfs") {
        let all_args: Vec<String> = std::env::args().collect();
        return crate::android::susfs::cli::run_from_args(&all_args);
    }

    let cli = Args::parse();

    log::info!("command: {:?}", cli.command);

    let result = match cli.command {
        Commands::AnyKernel3 { zip, slot } => anykernel3::flash(&zip, slot),
        Commands::Susfs(args) => crate::android::susfs::cli::run_main(args),
        Commands::PostFsData => init_event::on_post_fs_data(),
        Commands::BootCompleted => {
            init_event::on_boot_completed();
            Ok(())
        }
        Commands::UmountConfig { command } => match command {
            UmountConfigOp::Add { mnt, flags } => umount_config::add_umount(&mnt, flags),
            UmountConfigOp::Del { mnt } => umount_config::del_umount(&mnt),
            UmountConfigOp::Clear => umount_config::wipe_umount(),
            UmountConfigOp::List => umount_config::list_umount(),
        },
        Commands::SoftReboot => crate::android::soft_reboot::soft_reboot(),
        Commands::Insmod { module, params } => debug::insmod(&module, &params),
        Commands::Module { command } => {
            utils::switch_mnt_ns(1)?;
            match command {
                Module::Install { zip } => module::install_module(&zip),
                Module::UndoUninstall { id } => module::undo_uninstall_module(&id),
                Module::Uninstall { id } => module::uninstall_module(&id),
                Module::Enable { id } => module::enable_module(&id),
                Module::Disable { id } => module::disable_module(&id),
                Module::Action { id } => module::run_action(&id),
                Module::List => module::list_modules(),
                Module::Preinit { command } => match command {
                    PreinitCmd::Status => {
                        crate::android::preinit::print_status();
                        Ok(())
                    }
                    PreinitCmd::Reset => crate::android::preinit::reset_bootcount(),
                },
                Module::Config { internal, command } => {
                    let module_id = match internal {
                        Some(internal_name) => format!("internal.{internal_name}"),
                        None => std::env::var("KSU_MODULE").map_err(|_| {
                            anyhow::anyhow!(
                                "This command must be run in the context of a module or passed --internal <name>"
                            )
                        })?,
                    };
                    crate::android::module::validate_module_id(&module_id)?;

                    match command {
                        ModuleConfigCmd::Get { key } => {
                            // Use merge_configs to respect priority (temp overrides persist)
                            let config = module_config::merge_configs(&module_id)?;
                            match config.get(&key) {
                                Some(value) => {
                                    println!("{value}");
                                    Ok(())
                                }
                                None => anyhow::bail!("Key '{key}' not found"),
                            }
                        }
                        ModuleConfigCmd::Set {
                            key,
                            value,
                            stdin,
                            temp,
                        } => {
                            // Validate key at CLI layer for better user experience
                            module_config::validate_config_key(&key)?;

                            // Read value from stdin or argument
                            let value_str = match value {
                                Some(v) if !stdin => v,
                                _ => {
                                    // Read from stdin
                                    use std::io::Read;
                                    let mut buffer = String::new();
                                    std::io::stdin()
                                        .read_to_string(&mut buffer)
                                        .context("Failed to read from stdin")?;
                                    buffer
                                }
                            };

                            // Validate value
                            module_config::validate_config_value(&value_str)?;

                            let config_type = if temp {
                                module_config::ConfigType::Temp
                            } else {
                                module_config::ConfigType::Persist
                            };
                            module_config::set_config_value(
                                &module_id,
                                &key,
                                &value_str,
                                config_type,
                            )
                        }
                        ModuleConfigCmd::List => {
                            let config = module_config::merge_configs(&module_id)?;
                            if config.is_empty() {
                                println!("No config entries found");
                            } else {
                                for (key, value) in config {
                                    println!("{key}={value}");
                                }
                            }
                            Ok(())
                        }
                        ModuleConfigCmd::Delete { key, temp } => {
                            let config_type = if temp {
                                module_config::ConfigType::Temp
                            } else {
                                module_config::ConfigType::Persist
                            };
                            module_config::delete_config_value(&module_id, &key, config_type)
                        }
                        ModuleConfigCmd::Clear { temp } => {
                            let config_type = if temp {
                                module_config::ConfigType::Temp
                            } else {
                                module_config::ConfigType::Persist
                            };
                            module_config::clear_config(&module_id, config_type)
                        }
                    }
                }
            }
        }
        Commands::Install {
            libadbroot,
            data_path,
        } => utils::install(libadbroot, data_path),
        Commands::Unload => crate::android::unload::unload(),
        Commands::Uninstall { package_name } => utils::uninstall(&package_name),
        Commands::Sepolicy { command } => match command {
            Sepolicy::Patch { sepolicy } => sepolicy::live_patch(&sepolicy),
            Sepolicy::Apply { file } => sepolicy::apply_file(file),
            Sepolicy::Check { sepolicy } => sepolicy::check_rule(&sepolicy),
        },
        Commands::LateLoad {
            magica,
            allow_shell,
            post_magica,
            kmi,
            package_name,
        } => {
            if let Some(port) = magica {
                return crate::android::late_load::magica::run(port, &package_name, allow_shell)
                    .map_err(|e| {
                        error!("Error running magica: {e}");
                        e
                    });
            }
            let result = crate::android::late_load::run(&package_name, kmi, allow_shell);
            if post_magica {
                info!("Restoring adb properties (post-magica cleanup)...");
                if let Err(e) = crate::android::late_load::magica::disable_adb_root() {
                    error!("disable adb root failed: {e}");
                }
            }
            result
        }
        Commands::Services => {
            if ksucalls::get_version() <= 0 {
                info!("KernelSU not available, exiting services");
                std::process::exit(0);
            }
            init_event::on_services();
            Ok(())
        }
        Commands::Sulogd => sulog::run_sulogd(),
        Commands::Profile { command } => match command {
            Profile::GetSepolicy { package } => profile::get_sepolicy(package),
            Profile::SetSepolicy { package, policy } => profile::set_sepolicy(package, policy),
            Profile::GetTemplate { id } => profile::get_template(id),
            Profile::SetTemplate { id, template } => profile::set_template(id, template),
            Profile::DeleteTemplate { id } => profile::delete_template(id),
            Profile::ListTemplates { name, locale } => {
                profile::list_templates(name, locale.as_deref())
            }
        },

        Commands::Feature { command } => match command {
            Feature::Get { id, config } => {
                if config {
                    feature::get_feature_config(&id)
                } else {
                    feature::get_feature(&id)
                }
            }
            Feature::Set { id, value } => feature::set_feature(&id, value),
            Feature::List => {
                feature::list_features();
                Ok(())
            }
            Feature::Check { id } => feature::check_feature(&id),
            Feature::Load => feature::load_config_and_apply(),
            Feature::Save => feature::save_config(),
        },

        Commands::Debug { command } => match command {
            Debug::SetManager { apk } => debug::set_manager(&apk),
            Debug::GetSign { apk } => {
                let sign = apk_sign::get_apk_signature(&apk)?;
                println!("size: {:#x}, hash: {}", sign.0, sign.1);
                Ok(())
            }
            Debug::Version => {
                println!("Kernel Version: {}", ksucalls::get_version());
                Ok(())
            }
            Debug::Su { global_mnt } => su::grant_root(global_mnt),
            Debug::Test => assets::ensure_binaries(false),
            Debug::ExtractBinary { name, path } => {
                let data = assets::get_asset(&name)?;
                utils::ensure_binary(&path, data.as_ref(), false)
            }
            Debug::Mark { command } => match command {
                MarkCommand::Get { pid } => debug::mark_get(pid),
                MarkCommand::Mark { pid } => debug::mark_set(pid),
                MarkCommand::Unmark { pid } => debug::mark_unset(pid),
                MarkCommand::Refresh => debug::mark_refresh(),
            },
            Debug::Sulogd => sulog::ensure_sulogd_running(),
            Debug::Info => {
                let info = ksucalls::get_info();
                println!("version: {}", info.version);
                println!("full_version: {}", ksucalls::get_full_version());
                println!("flags: 0x{:x}", info.flags);
                println!("uapi_version: {}", info.uapi_version);
                println!("features: 0x{:x}", info.features);
                println!("lkm: {}", ksucalls::is_lkm());
                println!(
                    "bundled: {}",
                    (info.flags & uapi::KSU_GET_INFO_FLAG_BUNDLED) != 0
                );
                println!("late_load: {}", ksucalls::is_late_load());
                println!("runtime_mode: {}", ksucalls::runtime_mode());
                println!(
                    "pr_build: {}",
                    (info.flags & uapi::KSU_GET_INFO_FLAG_PR_BUILD) != 0
                );
                Ok(())
            }
            Debug::Package => {
                println!("{}", defs::DEFAULT_PACKAGE_NAME);
                Ok(())
            }
        },

        Commands::BootPatch(boot_patch) => crate::boot_patch::patch(boot_patch),

        Commands::BootInfo { command } => match command {
            BootInfo::CurrentKmi => {
                let kmi = crate::boot_patch::get_current_kmi()?;
                println!("{kmi}");
                // return here to avoid printing the error message
                return Ok(());
            }
            BootInfo::SupportedKmis => {
                let kmi = crate::assets::list_supported_kmi();
                for kmi in &kmi {
                    println!("{kmi}");
                }
                return Ok(());
            }
            BootInfo::IsAbDevice => {
                let val =
                    utils::getprop("ro.build.ab_update").unwrap_or_else(|| String::from("false"));
                let is_ab = val.trim().to_lowercase() == "true";
                println!("{}", if is_ab { "true" } else { "false" });
                return Ok(());
            }
            BootInfo::DefaultPartition => {
                let kmi = crate::boot_patch::get_current_kmi().unwrap_or_else(|_| String::new());
                let name = crate::boot_patch::choose_boot_partition(&kmi, false, &None);
                println!("{name}");
                return Ok(());
            }
            BootInfo::SlotSuffix { ota } => {
                let suffix = crate::boot_patch::get_slot_suffix(ota);
                println!("{suffix}");
                return Ok(());
            }
            BootInfo::AvailablePartitions => {
                let parts = crate::boot_patch::list_available_partitions();
                for p in &parts {
                    println!("{p}");
                }
                return Ok(());
            }
        },
        Commands::BootRestore(boot_restore) => crate::boot_patch::restore(boot_restore),
        Commands::Resetprop(resetprop_args) => crate::android::resetprop::run(&resetprop_args),
        Commands::BootPatchV2(patch) => crate::lkm_image::patch_boot(&patch),
        Commands::Kernel { command } => match command {
            Kernel::NukeExt4Sysfs { mnt } => ksucalls::nuke_ext4_sysfs(&mnt),
            Kernel::Umount { command } => match command {
                UmountOp::Add { mnt, flags } => ksucalls::umount_list_add(&mnt, flags),
                UmountOp::Del { mnt } => ksucalls::umount_list_del(&mnt),
                UmountOp::Wipe => ksucalls::umount_list_wipe(),
                UmountOp::List => {
                    let list = ksucalls::umount_list_list()?;
                    println!("{}", serde_json::to_string(&list)?);
                    Ok(())
                }
            },
            Kernel::DynamicManager { command } => match command {
                DynamicManagerOp::Set { size, hash } => dynamic_manager::set(size, hash),
                DynamicManagerOp::Get { internal } => {
                    let (size, hash) = ksucalls::dynamic_manager_get()?;
                    if internal.is_some_and(|s| s) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &serde_json::json!({"size":size,"hash":String::from_utf8_lossy(&hash)})
                            )?
                        );
                    } else {
                        println!("size: {}, hash: {}", size, String::from_utf8_lossy(&hash));
                    }
                    Ok(())
                }
                DynamicManagerOp::SetApk { apk } => {
                    let sign = apk_sign::get_apk_signature(&apk)?;

                    let bytes = sign.1.as_bytes();

                    let mut hash = [0u8; 64];
                    hash.copy_from_slice(bytes);

                    dynamic_manager::set(sign.0, hash)
                }
                DynamicManagerOp::Clear => dynamic_manager::clear(),
            },
            Kernel::NotifyModuleMounted => {
                ksucalls::report_module_mounted();
                Ok(())
            }
        },
        Commands::Initrc { command } => match command {
            Initrc::Refresh => regenerate_preinit(),
        },
        Commands::Tuning { command } => match command {
            TuningCmd::Status => {
                crate::android::tuning::print_status();
                Ok(())
            }
            TuningCmd::List => crate::android::tuning::list_profiles(),
            TuningCmd::Thermal { index } => crate::android::tuning::set_thermal_profile(index),
            TuningCmd::AppThermal { uid, index } => {
                crate::android::tuning::set_app_thermal(uid, index)
            }
            TuningCmd::AppThermalRemove { uid } => crate::android::tuning::remove_app_thermal(uid),
            TuningCmd::AppThermalClear => crate::android::tuning::clear_app_thermals(),
            TuningCmd::AppThermalList => {
                crate::android::tuning::print_app_thermals();
                Ok(())
            }
            TuningCmd::Watch => crate::android::tuning::run_tuning_watch(),
            TuningCmd::ChargeLimit { percent } => crate::android::tuning::set_charge_limit(percent),
            TuningCmd::ChargeLimitToggle { value } => {
                crate::android::tuning::set_charge_limit_enabled(value != 0)
            }
            TuningCmd::ChargeWritable { value } => {
                crate::android::tuning::set_charge_writable(value != 0)
            }
            TuningCmd::ChargeCurrent { ua } => crate::android::tuning::set_charge_current(ua),
            TuningCmd::Bypass { value } => crate::android::tuning::set_bypass(value != 0),
            TuningCmd::SchedSkipHeadroom { value } => {
                crate::android::tuning::set_sched_skip_headroom(value)
            }
            TuningCmd::SchedPeltMultiplier { value } => {
                crate::android::tuning::set_sched_pelt_multiplier(value)
            }
            TuningCmd::SchedHrScaling { value } => {
                crate::android::tuning::set_sched_hr_scaling(value)
            }
            TuningCmd::SchedBoostHrLp { value } => {
                crate::android::tuning::set_sched_boost_hr_lp(value)
            }
            TuningCmd::SchedBoostHrBig { value } => {
                crate::android::tuning::set_sched_boost_hr_big(value)
            }
            TuningCmd::SchedBoostHrPrime { value } => {
                crate::android::tuning::set_sched_boost_hr_prime(value)
            }
            TuningCmd::Cgroup { name, key, value } => {
                crate::android::tuning::set_cgroup_value(&name, &key, value)
            }
            TuningCmd::Persist { value } => crate::android::tuning::set_persist(value != 0),

            TuningCmd::TcpCongestion { value } => {
                crate::android::tuning::set_tcp_congestion(&value)
            }
            TuningCmd::Qdisc { value } => crate::android::tuning::set_qdisc(&value),

            TuningCmd::Swappiness { value } => crate::android::tuning::set_swappiness(value),
            TuningCmd::VfsCache { value } => crate::android::tuning::set_vfs_cache_pressure(value),
            TuningCmd::Kcompressd { value } => crate::android::tuning::set_kcompressd(value),
            TuningCmd::Mglru { value } => crate::android::tuning::set_mglru(value != 0),
            TuningCmd::MglruTtl { value } => crate::android::tuning::set_mglru_ttl(value),

            TuningCmd::ZramSize { gb } => crate::android::tuning::set_zram_size(gb),
            TuningCmd::ZramAlgorithm { algorithm } => {
                crate::android::tuning::set_zram_algorithm(&algorithm)
            }

            TuningCmd::Apply => {
                crate::android::tuning::apply_persisted();
                Ok(())
            }
        },

        Commands::Zygisk { command } => match command {
            ZygiskCommand::On => {
                crate::android::builtin_zygisk::switch_to(
                    crate::android::builtin_zygisk::BUILTIN_TARGET,
                )?;
                println!("true");
                Ok(())
            }
            ZygiskCommand::Off => {
                crate::android::builtin_zygisk::disable()?;
                println!("true");
                Ok(())
            }
            ZygiskCommand::Status => {
                let enabled = crate::android::builtin_zygisk::is_enabled();
                let monitor_running = crate::android::builtin_zygisk::is_monitor_active();
                let daemon_running = crate::android::builtin_zygisk::is_daemon_active();
                let zygote_injected = crate::android::builtin_zygisk::is_zygote_injected();
                let fully_active = crate::android::builtin_zygisk::is_fully_active();
                let zygotes: Vec<serde_json::Value> =
                    crate::android::builtin_zygisk::zygote_states()
                        .into_iter()
                        .map(|(abi, status)| serde_json::json!({ "abi": abi, "status": status }))
                        .collect();
                let monitor = crate::android::builtin_zygisk::monitor_process();
                let daemons = crate::android::builtin_zygisk::daemon_processes();
                let (version, version_code) = crate::android::builtin_zygisk::version()
                    .unwrap_or_else(|| ("unknown".to_owned(), "0".to_owned()));
                let monitor_json =
                    monitor.map(|(pid, name)| serde_json::json!({ "name": name, "pid": pid }));
                let daemons_json: Vec<_> = daemons
                    .into_iter()
                    .map(|(pid, name)| serde_json::json!({ "name": name, "pid": pid }))
                    .collect();
                let nm_active = crate::android::builtin_zygisk::is_nomount_active();
                let nm_isolated_block = if nm_active {
                    crate::android::builtin_zygisk::is_nomount_block_isolated()
                } else {
                    false
                };
                let nm_uids = if nm_active {
                    crate::android::builtin_zygisk::nomount_isolated_uids()
                } else {
                    Vec::new()
                };
                println!(
                    "{}",
                    serde_json::json!({
                        "enabled": enabled,
                        "monitor_running": monitor_running,
                        "daemon_running": daemon_running,
                        "zygote_injected": zygote_injected,
                        "fully_active": fully_active,
                        "zygotes": zygotes,
                        "monitor": monitor_json,
                        "daemons": daemons_json,
                        "version": version,
                        "version_code": version_code,
                        "nomount_active": nm_active,
                        "nomount_block_isolated": nm_isolated_block,
                        "nomount_isolated_uids_count": nm_uids.len(),
                        "nomount_isolated_uids": nm_uids,
                    })
                );
                Ok(())
            }
            ZygiskCommand::Info => {
                let info = crate::android::builtin_zygisk::implementation_info();
                println!(
                    "{}",
                    serde_json::json!({
                        "state": info.state,
                        "id": info.id,
                        "name": info.name,
                        "version": info.version,
                        "providers": crate::android::builtin_zygisk::provider_ids(),
                    })
                );
                Ok(())
            }
            ZygiskCommand::Switch { target } => {
                crate::android::builtin_zygisk::switch_to(&target)?;
                println!("true");
                Ok(())
            }
            ZygiskCommand::Processes { target } => {
                let procs = crate::android::builtin_zygisk::processes(&target)?;
                let list = |entries: Vec<(i32, String)>| {
                    entries
                        .into_iter()
                        .map(|(pid, name)| serde_json::json!({ "name": name, "pid": pid }))
                        .collect::<Vec<serde_json::Value>>()
                };
                println!(
                    "{}",
                    serde_json::json!({
                        "monitors": list(procs.monitors),
                        "daemons": list(procs.daemons),
                    })
                );
                Ok(())
            }
            ZygiskCommand::Modules => {
                let modules: Vec<serde_json::Value> =
                    crate::android::builtin_zygisk::injected_modules()
                        .into_iter()
                        .map(|module| {
                            let processes: Vec<serde_json::Value> = module
                                .processes
                                .into_iter()
                                .map(|(name, pid, kind)| {
                                    serde_json::json!({
                                        "name": name,
                                        "pid": pid,
                                        "kind": kind,
                                    })
                                })
                                .collect();
                            serde_json::json!({
                                "id": module.id,
                                "name": module.name,
                                "version": module.version,
                                "type": module.kind,
                                "abi": module.abi,
                                "targets": module.targets,
                                "companion": module.companion,
                                "state": module.state,
                                "processes": processes,
                            })
                        })
                        .collect();
                println!("{}", serde_json::json!({ "modules": modules }));
                Ok(())
            }
            ZygiskCommand::Rearm => {
                crate::android::builtin_zygisk::rearm()?;
                println!("true");
                Ok(())
            }
            ZygiskCommand::NomountSync => {
                crate::android::builtin_zygisk::coordinate_nomount()?;
                println!("true");
                Ok(())
            }
            ZygiskCommand::Config { command } => match command {
                ZygiskConfigCommand::Get => {
                    let fields: Vec<String> = crate::android::builtin_zygisk::config_get_all()
                        .into_iter()
                        .map(|(k, v)| format!(r#""{k}":"{v}""#))
                        .collect();
                    println!("{{{}}}", fields.join(","));
                    Ok(())
                }
                ZygiskConfigCommand::Set { key, value } => {
                    crate::android::builtin_zygisk::config_set(&key, &value)?;
                    println!("true");
                    Ok(())
                }
            },
        },
        Commands::Hma { command } => match command {
            HmaCommand::On => {
                crate::android::hma::enable()?;
                println!("true");
                Ok(())
            }
            HmaCommand::Off => {
                crate::android::hma::disable()?;
                println!("true");
                Ok(())
            }
            HmaCommand::Status => {
                let status = crate::android::hma::status();
                println!("{}", serde_json::to_string_pretty(&status)?);
                Ok(())
            }
            HmaCommand::Sync => {
                let count = crate::android::hma::sync_uid_scope()?;
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "success",
                        "synced_rules": count,
                    })
                );
                Ok(())
            }
            HmaCommand::Config { command } => match command {
                HmaConfigCommand::Get => {
                    println!("{}", crate::android::hma::get_config_json()?);
                    Ok(())
                }
                HmaConfigCommand::Set { key, value } => {
                    crate::android::hma::set_config_key(&key, &value)?;
                    println!("true");
                    Ok(())
                }
            },
            HmaCommand::Template { command } => match command {
                HmaTemplateCommand::List => {
                    println!("{}", crate::android::hma::list_templates_json()?);
                    Ok(())
                }
                HmaTemplateCommand::Get { name } => {
                    println!("{}", crate::android::hma::get_template_json(&name)?);
                    Ok(())
                }
                HmaTemplateCommand::Set {
                    name,
                    whitelist,
                    apps,
                } => {
                    crate::android::hma::set_template(&name, whitelist, &apps)?;
                    println!("true");
                    Ok(())
                }
                HmaTemplateCommand::Delete { name } => {
                    crate::android::hma::delete_template(&name)?;
                    println!("true");
                    Ok(())
                }
            },
            HmaCommand::Scope { command } => match command {
                HmaScopeCommand::List => {
                    println!("{}", crate::android::hma::list_scopes_json()?);
                    Ok(())
                }
                HmaScopeCommand::Get { package } => {
                    println!("{}", crate::android::hma::get_scope_json(&package)?);
                    Ok(())
                }
                HmaScopeCommand::Set {
                    package,
                    config_json,
                } => {
                    crate::android::hma::set_scope(&package, &config_json)?;
                    println!("true");
                    Ok(())
                }
                HmaScopeCommand::Remove { package } => {
                    crate::android::hma::remove_scope(&package)?;
                    println!("true");
                    Ok(())
                }
                HmaScopeCommand::Check { package } => {
                    println!("{}", crate::android::hma::check_scope(&package)?);
                    Ok(())
                }
                HmaScopeCommand::Clear => {
                    crate::android::hma::clear_scopes()?;
                    println!("true");
                    Ok(())
                }
            },
            HmaCommand::Preset { command } => match command {
                HmaPresetCommand::List => {
                    println!("{}", crate::android::hma::list_presets_json()?);
                    Ok(())
                }
                HmaPresetCommand::Get { name } => {
                    println!("{}", crate::android::hma::get_preset_json(&name)?);
                    Ok(())
                }
            },
            HmaCommand::SettingsTemplate { command } => match command {
                HmaSettingsTemplateCommand::List => {
                    println!("{}", crate::android::hma::list_settings_templates_json()?);
                    Ok(())
                }
                HmaSettingsTemplateCommand::Get { name } => {
                    println!(
                        "{}",
                        crate::android::hma::get_settings_template_json(&name)?
                    );
                    Ok(())
                }
                HmaSettingsTemplateCommand::Set { name, json } => {
                    crate::android::hma::set_settings_template(&name, &json)?;
                    println!("true");
                    Ok(())
                }
                HmaSettingsTemplateCommand::Delete { name } => {
                    crate::android::hma::delete_settings_template(&name)?;
                    println!("true");
                    Ok(())
                }
            },
            HmaCommand::Paths { command } => match command {
                HmaPathsCommand::Get => {
                    println!("{}", crate::android::hma::get_paths_config_json()?);
                    Ok(())
                }
                HmaPathsCommand::SetAuto { enabled } => {
                    let parsed = matches!(enabled.as_str(), "1" | "true" | "True" | "TRUE");
                    crate::android::hma::set_auto_hide_app_paths(parsed)?;
                    println!("true");
                    Ok(())
                }
                HmaPathsCommand::SetGlobal { paths } => {
                    crate::android::hma::set_global_paths(&paths)?;
                    println!("true");
                    Ok(())
                }
                HmaPathsCommand::SetApp { package, paths } => {
                    crate::android::hma::set_app_paths(&package, &paths)?;
                    println!("true");
                    Ok(())
                }
            },
        },
    };

    if let Err(e) = &result {
        log::error!("Error: {e:?}");
    }
    result
}
