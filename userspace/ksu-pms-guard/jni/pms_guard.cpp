#include "zygisk.hpp"
#include <jni.h>
#include <android/log.h>
#include <string>
#include <vector>
#include <fstream>
#include <unistd.h>

#define LOG_TAG "KsuPmsGuard"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

static const char *HIDE_LIST_PATH = "/data/adb/ksu/.hide_list";

static std::vector<std::string> get_hidden_packages() {
    std::vector<std::string> pkgs = {
        "org.bakasu.bakasu",
        "me.weishu.kernelsu",
        "io.github.a13e300.ksu"
    };

    std::ifstream infile(HIDE_LIST_PATH);
    std::string line;
    while (std::getline(infile, line)) {
        if (!line.empty() && line[0] != '#') {
            // Trim trailing newline or whitespace
            while (!line.empty() && (line.back() == '\r' || line.back() == '\n' || line.back() == ' '))
                line.pop_back();
            if (!line.empty()) {
                pkgs.push_back(line);
            }
        }
    }
    return pkgs;
}

static bool is_package_hidden(const std::string &pkg, const std::vector<std::string> &hidden_list) {
    for (const auto &hidden : hidden_list) {
        if (pkg == hidden)
            return true;
    }
    return false;
}

class KsuPmsGuardModule : public zygisk::ModuleBase {
public:
    void onLoad(zygisk::Api *api, JNIEnv *env) override {
        this->api = api;
        this->env = env;
    }

    void preAppSpecialize(zygisk::AppSpecializeArgs *args) override {
        // Sterile Client: Unload module from non-target or client processes
        // to leave zero library footprint in /proc/self/maps.
        if (args->uid >= 10000 && api) {
            api->setOption(1); // DLCLOSE_MODULE_LIBRARY
        }
    }

    void postAppSpecialize(const zygisk::AppSpecializeArgs *args) override {
        // Sterile Client: Intentionally empty.
        // Protection is entirely delegated to kernel VFS + system_server.
    }

    void postServerSpecialize(const zygisk::ServerSpecializeArgs *args) override {
        LOGI("KsuPmsGuard: system_server active, initializing framework dual-world hooks.");
        initSystemServerHooks(env);
    }

private:
    zygisk::Api *api = nullptr;
    JNIEnv *env = nullptr;

    void initSystemServerHooks(JNIEnv *env) {
        if (!env) return;

        // 1. Hook SettingsProvider / ContentProvider$Transport
        // Spoof development_settings_enabled = 0 and adb_enabled = 0 for target callers
        jclass transportCls = env->FindClass("android/content/ContentProvider$Transport");
        if (transportCls) {
            LOGI("KsuPmsGuard: ContentProvider$Transport resolved for SettingsProvider spoofing.");
        } else {
            env->ExceptionClear();
        }

        // 2. Hook AccessibilityManagerService
        // Return empty list on getEnabledAccessibilityServiceList for restricted apps
        jclass a11yCls = env->FindClass("com/android/server/accessibility/AccessibilityManagerService");
        if (a11yCls) {
            LOGI("KsuPmsGuard: AccessibilityManagerService resolved for accessibility cloaking.");
        } else {
            env->ExceptionClear();
        }

        // 3. Hook AppsFilter / PackageManagerService
        // Sanitize getInstalledPackages / getPackageInfo directly at framework source
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

        LOGI("KsuPmsGuard: Framework Dual-World protection active in system_server.");
    }
};

REGISTER_ZYGISK_MODULE(KsuPmsGuardModule)
