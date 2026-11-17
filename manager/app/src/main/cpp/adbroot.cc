#include <dlfcn.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <sys/system_properties.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <net/if.h>
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <netinet/tcp.h>

#ifndef PROP_VALUE_MAX
#define PROP_VALUE_MAX 92
#endif

#ifdef __cplusplus
extern "C" {
#endif

__attribute__((visibility("default"), used))
int __android_log_is_debuggable(void) {
    return 1;
}

struct prop_info {
    uint32_t dummy;
};

static struct prop_info g_adb_root_prop;
static struct prop_info g_adb_tcp_port_prop;

static const char* get_persistent_adb_port(void) {
    static char port_str[16] = "5555";
    static int loaded = 0;
    if (!loaded) {
        FILE *fp = fopen("/data/adb/ksu/.adb_port", "re");
        if (fp) {
            char buf[32] = {0};
            if (fgets(buf, sizeof(buf), fp)) {
                int p = atoi(buf);
                if (p > 0 && p <= 65535) {
                    snprintf(port_str, sizeof(port_str), "%d", p);
                }
            }
            fclose(fp);
        }
        loaded = 1;
    }
    return port_str;
}

__attribute__((visibility("default"), used))
const struct prop_info* __system_property_find(const char* name) {
    typedef const struct prop_info* (*find_fn_t)(const char*);
    static find_fn_t orig_find = NULL;
    if (name) {
        if (strcmp(name, "service.adb.root") == 0) {
            return &g_adb_root_prop;
        }
        if (strcmp(name, "service.adb.tcp.port") == 0) {
            return &g_adb_tcp_port_prop;
        }
    }
    if (!orig_find) {
        orig_find = (find_fn_t)dlsym(RTLD_NEXT, "__system_property_find");
    }
    return orig_find ? orig_find(name) : NULL;
}

__attribute__((visibility("default"), used))
void __system_property_read_callback(const struct prop_info* pi,
    void (*callback)(void* cookie, const char* name, const char* value, uint32_t serial),
    void* cookie) {
    typedef void (*read_cb_fn_t)(const struct prop_info*, void (*)(void*, const char*, const char*, uint32_t), void*);
    static read_cb_fn_t orig_fn = NULL;
    if (pi == &g_adb_root_prop) {
        if (callback) {
            callback(cookie, "service.adb.root", "1", 0);
        }
        return;
    }
    if (pi == &g_adb_tcp_port_prop) {
        if (callback) {
            callback(cookie, "service.adb.tcp.port", get_persistent_adb_port(), 0);
        }
        return;
    }
    if (!orig_fn) {
        orig_fn = (read_cb_fn_t)dlsym(RTLD_NEXT, "__system_property_read_callback");
    }
    if (orig_fn) {
        orig_fn(pi, callback, cookie);
    }
}

__attribute__((visibility("default"), used))
int __system_property_get(const char* name, char* value) {
    typedef int (*get_fn_t)(const char*, char*);
    static get_fn_t orig_fn = NULL;
    if (name && value) {
        if (strcmp(name, "service.adb.root") == 0) {
            strcpy(value, "1");
            return 1;
        }
        if (strcmp(name, "service.adb.tcp.port") == 0) {
            const char *p = get_persistent_adb_port();
            strncpy(value, p, PROP_VALUE_MAX);
            value[PROP_VALUE_MAX - 1] = '\0';
            return strlen(value);
        }
    }
    if (!orig_fn) {
        orig_fn = (get_fn_t)dlsym(RTLD_NEXT, "__system_property_get");
    }
    return orig_fn ? orig_fn(name, value) : 0;
}

static bool get_interface_ipv4(const char *ifname, struct in_addr *out_addr) {
    if (!ifname || !ifname[0]) return false;
    int sock = socket(AF_INET, SOCK_DGRAM, 0);
    if (sock < 0) return false;

    struct ifreq ifr;
    memset(&ifr, 0, sizeof(ifr));
    strncpy(ifr.ifr_name, ifname, IFNAMSIZ - 1);
    bool found = false;
    if (ioctl(sock, SIOCGIFADDR, &ifr) >= 0) {
        struct sockaddr_in *sin = (struct sockaddr_in *)&ifr.ifr_addr;
        if (out_addr) {
            *out_addr = sin->sin_addr;
        }
        found = true;
    }
    close(sock);
    return found;
}

static const char* get_tailscale_interface_name(struct in_addr *out_addr) {
    static const char *candidate_ifaces[] = { "dummy1", "tailscale0", NULL };
    for (int i = 0; candidate_ifaces[i] != NULL; ++i) {
        struct in_addr addr;
        memset(&addr, 0, sizeof(addr));
        if (get_interface_ipv4(candidate_ifaces[i], &addr)) {
            uint32_t ip = ntohl(addr.s_addr);
            if ((ip & 0xFFC00000) == 0x64400000) {
                if (out_addr) *out_addr = addr;
                return candidate_ifaces[i];
            }
        }
    }
    for (int i = 0; candidate_ifaces[i] != NULL; ++i) {
        if (if_nametoindex(candidate_ifaces[i]) > 0) {
            if (out_addr) get_interface_ipv4(candidate_ifaces[i], out_addr);
            return candidate_ifaces[i];
        }
    }
    return "dummy1";
}

__attribute__((visibility("default"), used))
int bind(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    typedef int (*bind_fn_t)(int, const struct sockaddr *, socklen_t);
    static bind_fn_t orig_bind = NULL;
    if (!orig_bind) {
        orig_bind = (bind_fn_t)dlsym(RTLD_NEXT, "bind");
    }

    if (addr && addr->sa_family == AF_INET && addrlen >= sizeof(struct sockaddr_in)) {
        const struct sockaddr_in *sin_in = (const struct sockaddr_in *)addr;
        int target_port = atoi(get_persistent_adb_port());

        if (ntohs(sin_in->sin_port) == target_port && sin_in->sin_addr.s_addr == htonl(INADDR_ANY)) {
            struct in_addr ts_addr;
            memset(&ts_addr, 0, sizeof(ts_addr));

            for (int retries = 0; retries < 30; ++retries) {
                struct in_addr probe_addr;
                memset(&probe_addr, 0, sizeof(probe_addr));
                get_tailscale_interface_name(&probe_addr);
                uint32_t ip = ntohl(probe_addr.s_addr);
                if ((ip & 0xFFC00000) == 0x64400000) {
                    ts_addr = probe_addr;
                    break;
                }
                usleep(100000); // 100ms
            }

            if (ts_addr.s_addr != 0) {
                struct sockaddr_in ts_sin = *sin_in;
                ts_sin.sin_addr = ts_addr; // Bind exclusively to Tailscale IP (dummy1)
                return orig_bind(sockfd, (const struct sockaddr *)&ts_sin, sizeof(ts_sin));
            }
        }
    }

    return orig_bind ? orig_bind(sockfd, addr, addrlen) : -1;
}

__attribute__((visibility("default"), used))
int execle(const char *pathname, const char *arg, ...) {
    const char *argv_list[256];
    int argc = 0;

    va_list va;
    va_start(va, arg);

    if (arg != NULL) {
        argv_list[argc++] = arg;
        const char *next;
        while ((next = va_arg(va, const char*)) != NULL && argc < 255) {
            argv_list[argc++] = next;
        }
    }
    argv_list[argc] = NULL;

    char *const *old_envp = va_arg(va, char* const*);
    va_end(va);

    const char *env_list[512];
    int envc = 0;
    bool ksurc_exists = (access("/data/adb/ksu/.ksurc", F_OK) == 0);
    const char *ksu_env_str = "ENV=/data/adb/ksu/.ksurc";

    if (old_envp != NULL) {
        for (size_t i = 0; old_envp[i] != NULL && envc < 500; ++i) {
            if (ksurc_exists && strncmp(old_envp[i], "ENV=", 4) == 0) {
                continue;
            }
            env_list[envc++] = old_envp[i];
        }
    }

    if (ksurc_exists && envc < 510) {
        env_list[envc++] = ksu_env_str;
    }
    env_list[envc] = NULL;

    return execve(pathname, (char *const *)argv_list, (char *const *)env_list);
}

__attribute__((visibility("default"), used))
int selinux_android_setcon(const char *con) {
    (void)con;
    return 0;
}

__attribute__((used, constructor(0)))
void Init(void) {
    unsetenv("LD_PRELOAD");
    unsetenv("LD_LIBRARY_PATH");
    const char *old_path = getenv("PATH");
    char new_path[1024];
    if (old_path && old_path[0]) {
        snprintf(new_path, sizeof(new_path), "%s:/data/adb/ksu/bin", old_path);
    } else {
        snprintf(new_path, sizeof(new_path), "/data/adb/ksu/bin");
    }
    setenv("PATH", new_path, 1);
}

#ifdef __cplusplus
}
#endif
