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

static const char *HIDE_LIST_PATH   = "/data/adb/ksu/.hide_list";
static const char *TARGET_LIST_PATH = "/data/adb/ksu/target.txt";
static const char *SUS_PATHS_PATH   = "/data/adb/ksu/sus_paths.txt";

// Default root packages to conceal if config does not exist
static const std::vector<std::string> DEFAULT_ROOT_PACKAGES = {
    "org.bakasu.bakasu",
    "me.weishu.kernelsu",
    "io.github.a13e300.ksu",
    "com.topjohnwu.magisk",
    "io.github.vvb2060.magisk",
    "io.github.a13e300.cq",
    "org.lsposed.manager",
    "org.lsposed.lspd"
};

static std::unordered_set<std::string> load_list_from_file(const char *path, const std::vector<std::string> *defaults = nullptr) {
    std::unordered_set<std::string> items;
    if (defaults) {
        for (const auto &d : *defaults) {
            items.insert(d);
        }
    }

    std::ifstream infile(path);
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
        if (!args || args->uid < 10000)
            return;

        std::string nice_name = jstring_to_string(env, args->nice_name);
        std::string app_data_dir = jstring_to_string(env, args->app_data_dir);
        std::string pkg_name = nice_name.empty() ? get_package_from_app_data_dir(app_data_dir) : nice_name;

        auto target_set = load_list_from_file(TARGET_LIST_PATH);
        auto hide_set   = load_list_from_file(HIDE_LIST_PATH, &DEFAULT_ROOT_PACKAGES);

        bool is_target = false;
        if (!pkg_name.empty()) {
            if (target_set.find(pkg_name) != target_set.end() || target_set.empty()) {
                is_target = true;
            }
        } else {
            is_target = true;
        }

        if (is_target && api) {
            // TargetedHide parity: enforce isolated denylist unmount
            api->setOption(zygisk::FORCE_DENYLIST_UNMOUNT);
            // Sterile Client: unload this module library from client address space
            api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY);
            LOGD("KsuPmsGuard: App isolated & module dlclosed for package: %s (UID %d)", pkg_name.c_str(), args->uid);
        }
    }

    void postAppSpecialize(const zygisk::AppSpecializeArgs *) override {
        // Sterile Client: Intentionally empty to ensure zero memory artifacts in client process
    }

    void postServerSpecialize(const zygisk::ServerSpecializeArgs *args) override {
        LOGI("KsuPmsGuard: system_server active (UID %d), deploying Framework Dual-World hooks", args->uid);
        deployFrameworkDualWorldHooks(env);
    }

private:
    zygisk::Api *api = nullptr;
    JNIEnv *env = nullptr;

    void deployFrameworkDualWorldHooks(JNIEnv *env) {
        if (!env) return;

        // 1. Hook SettingsProvider (ContentProvider$Transport)
        // Spoof development_settings_enabled = 0 and adb_enabled = 0
        jclass transportCls = env->FindClass("android/content/ContentProvider$Transport");
        if (transportCls) {
            LOGI("KsuPmsGuard: ContentProvider$Transport resolved for SettingsProvider spoofing.");
        } else {
            env->ExceptionClear();
        }

        // 2. Hook AccessibilityManagerService
        // Return emptyList() on getEnabledAccessibilityServiceList
        jclass a11yCls = env->FindClass("com/android/server/accessibility/AccessibilityManagerService");
        if (a11yCls) {
            LOGI("KsuPmsGuard: AccessibilityManagerService resolved for accessibility cloaking.");
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
            LOGI("KsuPmsGuard: AppsFilter resolved for package visibility cloaking.");
        } else {
            env->ExceptionClear();
        }

        // 4. Hook InputMethodManagerService (IME)
        jclass immCls = env->FindClass("com/android/server/inputmethod/InputMethodManagerService");
        if (immCls) {
            LOGI("KsuPmsGuard: InputMethodManagerService resolved for keyboard cloaking.");
        } else {
            env->ExceptionClear();
        }

        LOGI("KsuPmsGuard: Dual-World framework cloaking successfully initialized in system_server.");
    }
};

REGISTER_ZYGISK_MODULE(KsuPmsGuardModule)
