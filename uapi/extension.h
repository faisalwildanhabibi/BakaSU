#ifndef __KSU_UAPI_EXTENSION_H
#define __KSU_UAPI_EXTENSION_H

#include <linux/ioctl.h>
#include <linux/types.h>

#include "uapi/supercall.h"

enum ksu_extension_id {
    KSU_EXTENSION_KPM = 0,
    KSU_EXTENSION_CPU_SPOOF = 1,
    KSU_EXTENSION_UMOUNT_PATHS = 2,
    KSU_EXTENSION_DYNAMIC_MANAGER = 3,
    KSU_EXTENSION_SUSFS = 4,
    KSU_EXTENSION_VERSION_SPOOF = 5,

    KSU_EXTENSION_MAX
};

/* Operation discriminator for the unified KSU_IOCTL_EXTENSION. */
#define KSU_EXTENSION_GET  1u
#define KSU_EXTENSION_SET  2u
#define KSU_EXTENSION_IO   3u
#define KSU_EXTENSION_WIPE 4u
#define KSU_EXTENSION_ABI  5u
#define KSU_EXTENSION_ENUM 6u

#define KSU_EXTENSION_API_VERSION   2u
#define KSU_EXTENSION_ABI_PROBE_ID  0xFFFFFFFFu

#define KSU_EXTENSION_NAME_LEN 32

struct ksu_extension_desc {
    __u32 id;
    __u32 caps;
    __u32 min_api_version;
    __u32 forced;
    char name[KSU_EXTENSION_NAME_LEN];
};

#define KSU_EXTENSION_FLAG_STICKY   (1u << 1)
#define KSU_EXTENSION_FLAG_PERSIST  (1u << 2)
#define KSU_EXTENSION_FLAG_PER_UID  (1u << 3)
#define KSU_EXTENSION_FLAG_REBOOT   (1u << 4)
#define KSU_EXTENSION_FLAG_FORCED   (1u << 7)

struct ksu_extension_cmd {
    __u32 id;                  /* ksu_extension_id, in */
    __u32 op;                  /* KSU_EXTENSION_* in */
    __u32 supported;           /* capability bitfield, out */
    __u32 api_version;         /* extension ABI version, in/out */
    __u64 value;               /* sticky state bit, in/out */
    __u64 user_ptr;            /* opaque user pointer, in (for KSU_EXTENSION_IO) */
};

#ifndef KSU_IOCTL_EXTENSION
#define KSU_IOCTL_EXTENSION \
    _IOC(_IOC_READ | _IOC_WRITE, 'K', 300, sizeof(struct ksu_extension_cmd))
#endif

#endif /* __KSU_UAPI_EXTENSION_H */
