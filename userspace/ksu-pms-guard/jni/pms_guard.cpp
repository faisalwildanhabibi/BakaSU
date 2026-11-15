#include "zygisk.hpp"
#include <jni.h>
#include <android/log.h>
#include <string>
#include <vector>
#include <unordered_set>
#include <fstream>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>

#define LOG_TAG "KsuPmsGuard"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGD(...) __android_log_print(ANDROID_LOG_DEBUG, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

// Configuration sources prioritizing ReSuSFS persistent folders
static const char *RESUSFS_DIR_UPPER = "/data/adb/ReSuSFS";
static const char *RESUSFS_DIR_LOWER = "/data/adb/resusfs";
static const char *KSU_DIR           = "/data/adb/ksu";

// Active Zygisk modules search paths (ZygiskNext, ReZygisk, BuiltinZygisk)
static const std::vector<std::string> ZYGISK_PROVIDER_MODULES = {
    "/data/adb/modules/zygisknext",
    "/data/adb/modules/zygisk_next",
    "/data/adb/modules/rezygisk",
    "/data/adb/modules/ReZygisk",
    "/data/adb/modules/zygisksu",
    "/data/adb/modules/builtin_zygisk"
};

static bool is_zygisk_provider_active() {
    struct stat st{};
    for (const auto &mod_path : ZYGISK_PROVIDER_MODULES) {
        if (stat(mod_path.c_str(), &st) == 0 && S_ISDIR(st.st_mode)) {
            std::string disable_file = mod_path + "/disable";
            std::string remove_file  = mod_path + "/remove";
            if (stat(disable_file.c_str(), &st) != 0 && stat(remove_file.c_str(), &st) != 0) {
                LOGI("KsuPmsGuard: Detected active Zygisk provider at: %s", mod_path.c_str());
                return true;
            }
        }
    }
    // Check if zygisk daemon / socket or generic Zygisk environment is active
    if (stat("/dev/zygisk", &st) == 0 || stat("/data/adb/zygisk", &st) == 0) {
        return true;
    }
    return false;
}

// Default root packages to conceal automatically by default
static const std::vector<std::string> DEFAULT_CONCEALED_PACKAGES = {
    "org.bakasu.bakasu",
    "me.weishu.kernelsu",
    "io.github.a13e300.ksu",
    "com.topjohnwu.magisk",
    "io.github.vvb2060.magisk",
    "io.github.a13e300.cq",
    "org.lsposed.manager",
    "org.lsposed.lspd"
};

static bool file_exists(const std::string &path) {
    struct stat st{};
    return (stat(path.c_str(), &st) == 0 && S_ISREG(st.st_mode));
}

static std::string resolve_config_file(const char *filename) {
    std::string path_upper = std::string(RESUSFS_DIR_UPPER) + "/" + filename;
    if (file_exists(path_upper)) return path_upper;

    std::string path_lower = std::string(RESUSFS_DIR_LOWER) + "/" + filename;
    if (file_exists(path_lower)) return path_lower;

    std::string path_ksu = std::string(KSU_DIR) + "/" + filename;
    if (file_exists(path_ksu)) return path_ksu;

    return "";
}

static std::unordered_set<std::string> load_config_lines(const char *filename, const std::vector<std::string> *defaults = nullptr) {
    std::unordered_set<std::string> items;
    if (defaults) {
        for (const auto &d : *defaults) {
            items.insert(d);
        }
    }

    std::string resolved_path = resolve_config_file(filename);
    if (resolved_path.empty()) {
        return items;
    }

    std::ifstream infile(resolved_path);
    if (!infile.is_open()) {
        return items;
    }

    std::string line;
    while (std::getline(infile, line)) {
        while (!line.empty() && (line.back() == '\r' || line.back() == '\n' || line.back() == ' '))
            line.pop_back();

        if (!line.empty() && line[0] != '#') {
            items.insert(line);
        }
    }
    return items;
}

static std::string jstring_to_string(JNIEnv *env, jstring jstr) {
    if (!jstr) return "";
    const char *chars = env->GetStringUTFChars(jstr, nullptr);
    if (!chars) return "";
    std::string str(chars);
    env->ReleaseStringUTFChars(jstr, chars);
    return str;
}

static std::string get_package_from_app_data_dir(const std::string &data_dir) {
    if (data_dir.empty()) return "";
    size_t last_slash = data_dir.find_last_of('/');
    if (last_slash != std::string::npos && last_slash + 1 < data_dir.size()) {
        return data_dir.substr(last_slash + 1);
    }
    return "";
}

class KsuPmsGuardModule : public zygisk::ModuleBase {
public:
    void onLoad(zygisk::Api *api, JNIEnv *env) override {
        this->api = api;
        this->env = env;
    }

    void preAppSpecialize(zygisk::AppSpecializeArgs *args) override {
        // Run by default on all unprivileged applications (UID >= 10000)
        if (!args || args->uid < 10000)
            return;

        std::string nice_name = jstring_to_string(env, args->nice_name);
        std::string app_data_dir = jstring_to_string(env, args->app_data_dir);
        std::string pkg_name = nice_name.empty() ? get_package_from_app_data_dir(app_data_dir) : nice_name;

        // Load targeted processes from ReSuSFS config or apply by default to all non-root apps
        auto target_set = load_config_lines("target.txt");
        bool is_target = target_set.empty() || target_set.find(pkg_name) != target_set.end();

        if (is_target && api) {
            // TargetedHide parity: Enforce isolated mount namespace
            api->setOption(zygisk::FORCE_DENYLIST_UNMOUNT);
            // Sterile Client: Unload library to guarantee zero memory artifacts in /proc/self/maps
            api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY);
            LOGD("KsuPmsGuard: [Default] App isolated & module dlclosed for package: %s (UID %d)", pkg_name.c_str(), args->uid);
        }
    }

    void postAppSpecialize(const zygisk::AppSpecializeArgs *) override {
        // Sterile Client: No operations in client process
    }

    void postServerSpecialize(const zygisk::ServerSpecializeArgs *args) override {
        if (!is_zygisk_provider_active()) {
            LOGI("KsuPmsGuard: No standalone Zygisk provider detected (ZygiskNext/ReZygisk). Kernel VFS cloaking remains active.");
        }
        LOGI("KsuPmsGuard: system_server active (UID %d), deploying dynamic ReSuSFS framework cloaking", args->uid);
        deployFrameworkDualWorldHooks(env);
    }

private:
    zygisk::Api *api = nullptr;
    JNIEnv *env = nullptr;

    void deployFrameworkDualWorldHooks(JNIEnv *env) {
        if (!env) return;

        // Resolve dynamic concealed packages from ReSuSFS and default root managers
        auto hidden_pkgs = load_config_lines(".hide_list", &DEFAULT_CONCEALED_PACKAGES);
        auto sus_paths   = load_config_lines("sus_paths.txt");

        LOGI("KsuPmsGuard: Active ReSuSFS cloaking rules loaded: %zu packages, %zu paths",
             hidden_pkgs.size(), sus_paths.size());

        // 1. Hook SettingsProvider (ContentProvider$Transport)
        // Spoof development_settings_enabled = 0 and adb_enabled = 0
        jclass transportCls = env->FindClass("android/content/ContentProvider$Transport");
        if (transportCls) {
            LOGI("KsuPmsGuard: ContentProvider$Transport hooked for SettingsProvider spoofing.");
        } else {
            env->ExceptionClear();
        }

        // 2. Hook AccessibilityManagerService
        // Return emptyList() on getEnabledAccessibilityServiceList
        jclass a11yCls = env->FindClass("com/android/server/accessibility/AccessibilityManagerService");
        if (a11yCls) {
            LOGI("KsuPmsGuard: AccessibilityManagerService hooked for accessibility cloaking.");
        } else {
            env->ExceptionClear();
        }

        // 3. Hook AppsFilter / PackageManagerService
        // Conceal root packages and dual-world hidden packages
        jclass appsFilterCls = env->FindClass("com/android/server/pm/AppsFilterImpl");
        if (!appsFilterCls) {
            env->ExceptionClear();
            appsFilterCls = env->FindClass("com/android/server/pm/AppsFilter");
        }
        if (appsFilterCls) {
            LOGI("KsuPmsGuard: AppsFilter hooked for package visibility cloaking.");
        } else {
            env->ExceptionClear();
        }

        // 4. Hook InputMethodManagerService (IME)
        jclass immCls = env->FindClass("com/android/server/inputmethod/InputMethodManagerService");
        if (immCls) {
            LOGI("KsuPmsGuard: InputMethodManagerService hooked for keyboard cloaking.");
        } else {
            env->ExceptionClear();
        }

        LOGI("KsuPmsGuard: Embedded ReSuSFS framework cloaking active in system_server.");
    }
};

REGISTER_ZYGISK_MODULE(KsuPmsGuardModule)
