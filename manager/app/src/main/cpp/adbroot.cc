#include <string>
#include <dlfcn.h>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <unistd.h>
#include <vector>
#include <sys/system_properties.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <net/if.h>
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <netinet/tcp.h>

// Skip drop privileges
// https://cs.android.com/android/platform/superproject/+/android-latest-release:packages/modules/adb/daemon/main.cpp;l=123;drc=3b74954ec50836e0c8faeed1877fefb1dc2de006
extern "C"
[[gnu::visibility("default"), gnu::used]]
// NOLINTNEXTLINE(bugprone-reserved-identifier)
int __android_log_is_debuggable() {
    return 1;
}

struct prop_info {};

static struct prop_info g_adb_root_prop;
static struct prop_info g_adb_tcp_port_prop;

static const std::string& get_persistent_adb_port() {
    static std::string port_str;
    static bool loaded = false;
    if (!loaded) {
        FILE *fp = fopen("/data/adb/ksu/.adb_port", "re");
        if (fp) {
            char buf[32] = {0};
            if (fgets(buf, sizeof(buf), fp)) {
                int p = atoi(buf);
                if (p > 0 && p <= 65535) {
                    port_str = std::to_string(p);
                }
            }
            fclose(fp);
        }
        if (port_str.empty()) {
            port_str = "5555";
        }
        loaded = true;
    }
    return port_str;
}

// Enable adb root and persistent wireless port by default, without running `adb root` or setprop
// https://cs.android.com/android/platform/superproject/+/android-latest-release:packages/modules/adb/daemon/main.cpp;l=84;drc=3b74954ec50836e0c8faeed1877fefb1dc2de006
// https://cs.android.com/android/platform/superproject/+/android-latest-release:system/libbase/properties.cpp;l=159-175;drc=6d19b5c690fa4220c10e42c2326150bbd4d4b7bb
// android::base::GetProperty is from libbase.so, but some adbd are statically-linked to it, 
// so we replace these underlying functions.

extern "C" [[gnu::visibility("default"), gnu::used]]
const prop_info* __system_property_find(const char* name) {
    static decltype(__system_property_find) *system_property_find_fn = nullptr;
    if (name) {
        if (strcmp(name, "service.adb.root") == 0) {
            return &g_adb_root_prop;
        }
        if (strcmp(name, "service.adb.tcp.port") == 0) {
            return &g_adb_tcp_port_prop;
        }
    }
    if (!system_property_find_fn) {
        system_property_find_fn = (decltype(system_property_find_fn)) dlsym(RTLD_NEXT, "__system_property_find");
    }
    return system_property_find_fn(name);
}

extern "C" [[gnu::visibility("default"), gnu::used]]
void __system_property_read_callback(const prop_info* pi,
    void (*callback)(void* cookie, const char* name, const char* value, uint32_t serial),
    void* cookie) {
    static decltype(__system_property_read_callback) *orig_fn = nullptr;
    if (pi == &g_adb_root_prop) {
        if (callback) {
            callback(cookie, "service.adb.root", "1", 0);
        }
        return;
    }
    if (pi == &g_adb_tcp_port_prop) {
        if (callback) {
            callback(cookie, "service.adb.tcp.port", get_persistent_adb_port().c_str(), 0);
        }
        return;
    }
    if (!orig_fn) {
        orig_fn = (decltype(orig_fn)) dlsym(RTLD_NEXT, "__system_property_read_callback");
    }
    orig_fn(pi, callback, cookie);
}

extern "C" [[gnu::visibility("default"), gnu::used]]
int __system_property_get(const char* name, char* value) {
    static decltype(__system_property_get) *orig_fn = nullptr;
    if (name && value) {
        if (strcmp(name, "service.adb.root") == 0) {
            strcpy(value, "1");
            return 1;
        }
        if (strcmp(name, "service.adb.tcp.port") == 0) {
            const std::string& p = get_persistent_adb_port();
            strncpy(value, p.c_str(), PROP_VALUE_MAX);
            value[PROP_VALUE_MAX - 1] = '\0';
            return strlen(value);
        }
    }
    if (!orig_fn) {
        orig_fn = (decltype(orig_fn)) dlsym(RTLD_NEXT, "__system_property_get");
    }
    return orig_fn(name, value);
}

static bool is_tailscale_environment_present() {
    static int cached_state = -1;
    if (cached_state == -1) {
        if (access("/data/adb/tailscale/tailscaled", X_OK) == 0 ||
            access("/data/adb/tailscale", F_OK) == 0) {
            cached_state = 1;
        } else {
            cached_state = 0;
        }
    }
    return cached_state == 1;
}

static bool get_interface_ipv4(const char *ifname, struct in_addr *out_addr) {
    if (!ifname || !ifname[0]) return false;
    int sock = socket(AF_INET, SOCK_DGRAM, 0);
    if (sock < 0) return false;

    struct ifreq ifr{};
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
    // Prioritas 1: Antarmuka kamuflase dummy1
    // Prioritas 2: Antarmuka standar tailscale0
    static const char *candidate_ifaces[] = { "dummy1", "tailscale0", nullptr };
    for (int i = 0; candidate_ifaces[i] != nullptr; ++i) {
        struct in_addr addr{};
        if (get_interface_ipv4(candidate_ifaces[i], &addr)) {
            uint32_t ip = ntohl(addr.s_addr);
            // Validasi subnet CGNAT Tailscale: 100.64.0.0/10
            if ((ip & 0xFFC00000) == 0x64400000) {
                if (out_addr) *out_addr = addr;
                return candidate_ifaces[i];
            }
        }
    }
    for (int i = 0; candidate_ifaces[i] != nullptr; ++i) {
        if (if_nametoindex(candidate_ifaces[i]) > 0) {
            if (out_addr) get_interface_ipv4(candidate_ifaces[i], out_addr);
            return candidate_ifaces[i];
        }
    }
    return "dummy1";
}

extern "C" [[gnu::visibility("default"), gnu::used]]
int bind(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    static decltype(bind) *orig_bind = nullptr;
    if (!orig_bind) {
        orig_bind = (decltype(orig_bind)) dlsym(RTLD_NEXT, "bind");
    }

    if (addr && addr->sa_family == AF_INET && addrlen >= sizeof(struct sockaddr_in)) {
        const struct sockaddr_in *sin_in = (const struct sockaddr_in *)addr;
        int target_port = atoi(get_persistent_adb_port().c_str());

        // Intercept adbd TCP listener on persistent port
        if (ntohs(sin_in->sin_port) == target_port && sin_in->sin_addr.s_addr == htonl(INADDR_ANY)) {
            struct in_addr ts_addr{};
            const char *ts_ifname = nullptr;

            // Wait up to 3 seconds for Tailscale interface (dummy1 / tailscale0) and IP readiness
            for (int retries = 0; retries < 30; ++retries) {
                struct in_addr probe_addr{};
                const char *candidate = get_tailscale_interface_name(&probe_addr);
                uint32_t ip = ntohl(probe_addr.s_addr);
                if ((ip & 0xFFC00000) == 0x64400000) {
                    ts_addr = probe_addr;
                    ts_ifname = candidate;
                    break;
                }
                usleep(100000); // 100ms
            }

            if (ts_ifname) {
                struct sockaddr_in ts_sin = *sin_in;
                ts_sin.sin_addr = ts_addr; // Bind exclusively to Tailscale IP (dummy1)
                setsockopt(sockfd, SOL_SOCKET, SO_BINDTODEVICE, ts_ifname, strlen(ts_ifname));
                return orig_bind(sockfd, (const struct sockaddr *)&ts_sin, sizeof(ts_sin));
            }
        }
    }

    return orig_bind(sockfd, addr, addrlen);
}

static bool is_connection_allowed(int fd) {
    int target_port = atoi(get_persistent_adb_port().c_str());
    struct sockaddr_storage local_addr{};
    socklen_t local_len = sizeof(local_addr);

    if (getsockname(fd, (struct sockaddr *)&local_addr, &local_len) == 0) {
        if (local_addr.ss_family == AF_INET) {
            struct sockaddr_in *sin = (struct sockaddr_in *)&local_addr;
            if (ntohs(sin->sin_port) == target_port) {
                // Deny loopback IPv4 (127.0.0.1) so local security scanners immediately get ECONNREFUSED
                if (sin->sin_addr.s_addr == htonl(INADDR_LOOPBACK)) {
                    return false;
                }

                // 1. Check exact match with Tailscale interface IP (dummy1 / tailscale0)
                struct in_addr ts_addr{};
                get_tailscale_interface_name(&ts_addr);
                if (sin->sin_addr.s_addr == ts_addr.s_addr) {
                    return true;
                }

                // 2. Allow Tailscale CGNAT subnet (100.64.0.0/10)
                uint32_t s_ip = ntohl(sin->sin_addr.s_addr);
                if ((s_ip & 0xFFC00000) == 0x64400000) {
                    return true;
                }

                // Deny all other interfaces (Wi-Fi, cellular rmnet*, etc.)
                return false;
            }
        } else if (local_addr.ss_family == AF_INET6) {
            struct sockaddr_in6 *sin6 = (struct sockaddr_in6 *)&local_addr;
            if (ntohs(sin6->sin6_port) == target_port) {
                // Allow Tailscale ULA IPv6 range (fd7a:115c:a1e0::/48)
                const uint8_t *addr6 = sin6->sin6_addr.s6_addr;
                if (addr6[0] == 0xfd && addr6[1] == 0x7a &&
                    addr6[2] == 0x11 && addr6[3] == 0x5c &&
                    addr6[4] == 0xa1 && addr6[5] == 0xe0) {
                    return true;
                }

                // Deny loopback IPv6 (::1) and external public IPv6
                return false;
            }
        }
    }
    return true;
}

static void apply_tcp_keepalive(int fd) {
    if (fd < 0) return;
    int optval = 1;
    setsockopt(fd, SOL_SOCKET, SO_KEEPALIVE, &optval, sizeof(optval));
    int keepidle = 15;  // 15 seconds idle before sending keepalive probe
    int keepintvl = 5;  // 5 seconds between probes
    int keepcnt = 3;    // 3 probes before dropping dead socket
#ifdef TCP_KEEPIDLE
    setsockopt(fd, IPPROTO_TCP, TCP_KEEPIDLE, &keepidle, sizeof(keepidle));
#endif
#ifdef TCP_KEEPINTVL
    setsockopt(fd, IPPROTO_TCP, TCP_KEEPINTVL, &keepintvl, sizeof(keepintvl));
#endif
#ifdef TCP_KEEPCNT
    setsockopt(fd, IPPROTO_TCP, TCP_KEEPCNT, &keepcnt, sizeof(keepcnt));
#endif
}

extern "C" [[gnu::visibility("default"), gnu::used]]
int accept4(int sockfd, struct sockaddr *addr, socklen_t *addrlen, int flags) {
    static decltype(accept4) *orig_accept4 = nullptr;
    if (!orig_accept4) {
        orig_accept4 = (decltype(orig_accept4)) dlsym(RTLD_NEXT, "accept4");
    }
    int fd = orig_accept4(sockfd, addr, addrlen, flags);
    if (fd >= 0) {
        if (!is_connection_allowed(fd)) {
            close(fd);
            errno = ECONNREFUSED;
            return -1;
        }
        // Force accepted socket to bind to Tailscale disguised interface (dummy1)
        struct in_addr probe_addr{};
        const char *ts_ifname = get_tailscale_interface_name(&probe_addr);
        setsockopt(fd, SOL_SOCKET, SO_BINDTODEVICE, ts_ifname, strlen(ts_ifname));
        if (addr && (addr->sa_family == AF_INET || addr->sa_family == AF_INET6)) {
            apply_tcp_keepalive(fd);
        }
    }
    return fd;
}

extern "C" [[gnu::visibility("default"), gnu::used]]
int accept(int sockfd, struct sockaddr *addr, socklen_t *addrlen) {
    static decltype(accept) *orig_accept = nullptr;
    if (!orig_accept) {
        orig_accept = (decltype(orig_accept)) dlsym(RTLD_NEXT, "accept");
    }
    int fd = orig_accept(sockfd, addr, addrlen);
    if (fd >= 0) {
        if (!is_connection_allowed(fd)) {
            close(fd);
            errno = ECONNREFUSED;
            return -1;
        }
        // Force accepted socket to bind to Tailscale disguised interface (dummy1)
        struct in_addr probe_addr{};
        const char *ts_ifname = get_tailscale_interface_name(&probe_addr);
        setsockopt(fd, SOL_SOCKET, SO_BINDTODEVICE, ts_ifname, strlen(ts_ifname));
        if (addr && (addr->sa_family == AF_INET || addr->sa_family == AF_INET6)) {
            apply_tcp_keepalive(fd);
        }
    }
    return fd;
}

// If the user creates ksurc, use ksurc instead of mkshrc
// https://cs.android.com/android/platform/superproject/+/android-latest-release:packages/modules/adb/daemon/shell_service.cpp;l=389-394
extern "C" [[gnu::visibility("default"), gnu::used]]
int execle(const char *pathname, const char *arg, ...) {
    std::vector<const char *> argv_list;
    va_list va;
    va_start(va, arg);

    // start dump argv
    if (arg != nullptr) {
        argv_list.push_back(arg);
        const char *next;
        while ((next = va_arg(va, const char*)) != nullptr) {
            argv_list.push_back(next);
        }
    }
    argv_list.push_back(nullptr);

    char *const *old_envp = va_arg(va, char* const*);
    va_end(va);

    // start dump envp
    std::vector<const char *> env_list;
    bool ksurc_exists = (access("/data/adb/ksu/.ksurc", F_OK) == 0);
    const char *ksu_env_str = "ENV=/data/adb/ksu/.ksurc";

    if (old_envp != nullptr) {
        for (size_t i = 0; old_envp[i] != nullptr; ++i) {
            // skip original ENV in envp
            // even there shouldn't exists in normal android, but let's still check it, avoid newer android/custom rom modify there
            if (ksurc_exists && strncmp(old_envp[i], "ENV=", 4) == 0) {
                continue;
            }
            env_list.push_back(old_envp[i]);
        }
    }

    if (ksurc_exists) {
        // push our ENV in here!!!
        env_list.push_back(ksu_env_str);
    }
    env_list.push_back(nullptr);

    // skip origin execle, because we already completed everything of execle
    return execve(pathname,
                  const_cast<char *const *>(argv_list.data()),
                  const_cast<char *const *>(env_list.data()));
}

// skip setcon
extern "C"
[[gnu::visibility("default"), gnu::used]]
int selinux_android_setcon(const char *con) {
    return 0;
}

[[gnu::used, gnu::constructor(0)]]
void Init() {
    unsetenv("LD_PRELOAD");
    unsetenv("LD_LIBRARY_PATH");
    std::string path = getenv("PATH") ?: "";
    if (!path.empty()) {
        path += ":/data/adb/ksu/bin";
    } else {
        path += "/data/adb/ksu/bin";
    }
    setenv("PATH", path.c_str(), 1);
}
