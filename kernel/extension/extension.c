#include <linux/errno.h>
#include <linux/kernel.h>
#include <linux/mutex.h>
#include <linux/string.h>
#include <linux/types.h>
#include <linux/uaccess.h>

#include "extension.h"
#include "klog.h"
#include "feature/dynamic_manager.h"
#include "uapi/supercall.h"

#ifdef CONFIG_KSU_SUSFS
#include <linux/susfs.h>
#endif

static const struct ksu_extension_handler *extension_handlers[KSU_EXTENSION_MAX];
static DEFINE_MUTEX(extension_mutex);

static const char *const extension_names[KSU_EXTENSION_MAX] = {
    [KSU_EXTENSION_KPM] = "kpm",
    [KSU_EXTENSION_CPU_SPOOF] = "cpu_spoof",
    [KSU_EXTENSION_UMOUNT_PATHS] = "umount_paths",
    [KSU_EXTENSION_DYNAMIC_MANAGER] = "dynamic_manager",
    [KSU_EXTENSION_SUSFS] = "susfs",
    [KSU_EXTENSION_VERSION_SPOOF] = "version_spoof",
};

static bool extension_api_allowed(const struct ksu_extension_handler *handler, u32 caller_api_version)
{
    u32 caller = caller_api_version ? caller_api_version : 1;
    return !handler->min_api_version || caller >= handler->min_api_version;
}

int ksu_register_extension_handler(const struct ksu_extension_handler *handler)
{
    if (!handler || handler->extension_id >= KSU_EXTENSION_MAX)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    extension_handlers[handler->extension_id] = handler;
    pr_info("extension: registered handler for %s (id=%u, caps=0x%x)\n",
            handler->name ? handler->name : "unknown", handler->extension_id, handler->caps);
    mutex_unlock(&extension_mutex);
    return 0;
}

int ksu_unregister_extension_handler(u32 extension_id)
{
    if (extension_id >= KSU_EXTENSION_MAX)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    extension_handlers[extension_id] = NULL;
    mutex_unlock(&extension_mutex);
    return 0;
}

int ksu_extension_get(u32 extension_id, u32 caller_api_version, u64 *value, u32 *caps)
{
    const struct ksu_extension_handler *handler;
    int ret = 0;

    if (extension_id >= KSU_EXTENSION_MAX || !value || !caps)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    handler = extension_handlers[extension_id];
    *caps = 0;
    *value = 0;
    if (!handler)
        goto out;

    if (!extension_api_allowed(handler, caller_api_version))
        goto out;

    *caps = KSU_FEATURE_FLAG_SUPPORTED | handler->caps;
    if (handler->ops && handler->ops->get) {
        ret = handler->ops->get(extension_id, value);
        if (ret)
            *caps = 0;
    }
out:
    mutex_unlock(&extension_mutex);
    return ret;
}

int ksu_extension_set(u32 extension_id, u32 caller_api_version, u64 value)
{
    const struct ksu_extension_handler *handler;
    int ret = 0;

    if (extension_id >= KSU_EXTENSION_MAX)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    handler = extension_handlers[extension_id];
    if (!handler || !handler->ops || !handler->ops->set) {
        ret = -EOPNOTSUPP;
        goto out;
    }

    if (!extension_api_allowed(handler, caller_api_version)) {
        ret = -EOPNOTSUPP;
        goto out;
    }

    ret = handler->ops->set(extension_id, value);
out:
    mutex_unlock(&extension_mutex);
    return ret;
}

int ksu_extension_enumerate(u32 index, struct ksu_extension_desc *desc)
{
    const struct ksu_extension_handler *handler;
    u32 id, seen = 0;
    int ret = -ENOENT;

    if (!desc)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    for (id = 0; id < KSU_EXTENSION_MAX; id++) {
        handler = extension_handlers[id];
        if (!handler)
            continue;
        if (seen++ != index)
            continue;

        memset(desc, 0, sizeof(*desc));
        desc->id = id;
        desc->caps = KSU_FEATURE_FLAG_SUPPORTED | handler->caps;
        desc->min_api_version = handler->min_api_version;
        desc->forced = 0;
        strncpy(desc->name, handler->name ? handler->name : "unknown", sizeof(desc->name) - 1);
        ret = 0;
        break;
    }
    mutex_unlock(&extension_mutex);
    return ret;
}

int ksu_extension_ioctl(u32 extension_id, u32 caller_api_version, void __user *raw)
{
    const struct ksu_extension_handler *handler;
    int ret;

    if (extension_id >= KSU_EXTENSION_MAX)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    handler = extension_handlers[extension_id];
    if (!handler || !handler->io_handler || !extension_api_allowed(handler, caller_api_version)) {
        ret = -EOPNOTSUPP;
        goto out;
    }
    ret = handler->io_handler(extension_id, raw);
out:
    mutex_unlock(&extension_mutex);
    return ret;
}

int ksu_extension_wipe(u32 extension_id, u32 caller_api_version)
{
    const struct ksu_extension_handler *handler;
    int ret = 0;

    if (extension_id >= KSU_EXTENSION_MAX)
        return -EINVAL;

    mutex_lock(&extension_mutex);
    handler = extension_handlers[extension_id];
    if (!handler || !handler->wipe || !extension_api_allowed(handler, caller_api_version)) {
        ret = -EOPNOTSUPP;
        goto out;
    }
    ret = handler->wipe(extension_id);
out:
    mutex_unlock(&extension_mutex);
    return ret;
}

// Extension handlers:
// 1. Dynamic Manager
static int dynamic_manager_get(u32 id, u64 *value)
{
    *value = ksu_is_dynamic_manager_enabled() ? 1 : 0;
    return 0;
}

static int dynamic_manager_set(u32 id, u64 value)
{
    if (value == 0) {
        struct ksu_dynamic_manager_cmd cmd = {
            .operation = DYNAMIC_MANAGER_OP_WIPE,
        };
        return ksu_handle_dynamic_manager(&cmd);
    }
    return 0;
}

static const struct ksu_extension_ops dynamic_manager_ops = {
    .get = dynamic_manager_get,
    .set = dynamic_manager_set,
};

// 2. SuSFS
static int susfs_extension_get(u32 id, u64 *value)
{
#ifdef CONFIG_KSU_SUSFS
    *value = 1;
#else
    *value = 0;
#endif
    return 0;
}

static const struct ksu_extension_ops susfs_extension_ops = {
    .get = susfs_extension_get,
    .set = NULL,
};

// 3. Umount Paths
static int umount_paths_get(u32 id, u64 *value)
{
    *value = 1;
    return 0;
}

static const struct ksu_extension_ops umount_paths_ops = {
    .get = umount_paths_get,
    .set = NULL,
};

void __init ksu_extension_init(void)
{
    int i;
    for (i = 0; i < KSU_EXTENSION_MAX; i++)
        extension_handlers[i] = NULL;
    pr_info("extension: subsystem initialized\n");
}

void __init ksu_extension_register_all(void)
{
    static const struct ksu_extension_handler dynamic_manager_handler = {
        .extension_id = KSU_EXTENSION_DYNAMIC_MANAGER,
        .name = "dynamic_manager",
        .caps = KSU_EXTENSION_FLAG_STICKY | KSU_EXTENSION_FLAG_PERSIST,
        .ops = &dynamic_manager_ops,
    };

    static const struct ksu_extension_handler susfs_handler = {
        .extension_id = KSU_EXTENSION_SUSFS,
        .name = "susfs",
        .caps = KSU_EXTENSION_FLAG_REBOOT,
        .ops = &susfs_extension_ops,
    };

    static const struct ksu_extension_handler umount_paths_handler = {
        .extension_id = KSU_EXTENSION_UMOUNT_PATHS,
        .name = "umount_paths",
        .caps = 0,
        .ops = &umount_paths_ops,
    };

    ksu_register_extension_handler(&dynamic_manager_handler);
    ksu_register_extension_handler(&susfs_handler);
    ksu_register_extension_handler(&umount_paths_handler);
}

void __exit ksu_extension_exit(void)
{
    mutex_lock(&extension_mutex);
    memset(extension_handlers, 0, sizeof(extension_handlers));
    mutex_unlock(&extension_mutex);
}
