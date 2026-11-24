#ifndef __KSU_EXTENSION_H
#define __KSU_EXTENSION_H

#include <linux/types.h>
#include "uapi/extension.h"

struct ksu_extension_handler;

struct ksu_extension_ops {
    int (*get)(u32 extension_id, u64 *value);
    int (*set)(u32 extension_id, u64 value);
};

struct ksu_extension_handler {
    u32 extension_id;
    const char *name;
    u32 caps;
    u32 min_api_version;
    const struct ksu_extension_ops *ops;
    int (*io_handler)(u32 extension_id, void __user *arg);
    int (*wipe)(u32 extension_id);
};

int ksu_register_extension_handler(const struct ksu_extension_handler *handler);
int ksu_unregister_extension_handler(u32 extension_id);
int ksu_extension_get(u32 extension_id, u32 caller_api_version, u64 *value, u32 *caps);
int ksu_extension_set(u32 extension_id, u32 caller_api_version, u64 value);
int ksu_extension_enumerate(u32 index, struct ksu_extension_desc *desc);
int ksu_extension_ioctl(u32 extension_id, u32 caller_api_version, void __user *raw);
int ksu_extension_wipe(u32 extension_id, u32 caller_api_version);

void ksu_extension_init(void);
void ksu_extension_register_all(void);
void ksu_extension_exit(void);

#endif /* __KSU_EXTENSION_H */
