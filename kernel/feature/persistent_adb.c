#include <linux/cred.h>
#include <linux/delay.h>
#include <linux/err.h>
#include <linux/init.h>
#include <linux/kernel.h>
#include <linux/proc_fs.h>
#include <linux/seq_file.h>
#include <linux/slab.h>
#include <linux/string.h>
#include <linux/types.h>
#include <linux/umh.h>
#include <linux/workqueue.h>
#include <net/inet_hashtables.h>
#include <net/inet_sock.h>
#include <net/net_namespace.h>
#include <net/sock.h>
#include <net/tcp.h>
#include <net/timewait_sock.h>

#include "persistent_adb.h"
#include "hook/patch_memory.h"
#include "infra/symbol_resolver.h"
#include "klog.h"
#include "ksu.h"

u16 ksu_persistent_adb_port = DEFAULT_PERSISTENT_ADB_PORT;

static struct work_struct persistent_adb_work;

static int (*orig_tcp4_seq_show)(struct seq_file *seq, void *v) = NULL;
static int (*orig_tcp6_seq_show)(struct seq_file *seq, void *v) = NULL;

static void **tcp4_seq_show_slot = NULL;
static void **tcp6_seq_show_slot = NULL;

static bool is_adb_port_listening(u16 port)
{
    struct sock *sk = inet_lookup_listener(&init_net, &tcp_hashinfo, NULL, 0, 0, 0, 0, htons(port), 0, 0);
    if (sk) {
        sock_put(sk);
        return true;
    }
    return false;
}

static int hook_tcp4_seq_show(struct seq_file *seq, void *v)
{
    if (v != SEQ_START_TOKEN) {
        struct tcp_iter_state *st = seq ? (struct tcp_iter_state *)seq->private : NULL;
        if (st && (st->state == TCP_SEQ_STATE_LISTENING || st->state == TCP_SEQ_STATE_ESTABLISHED)) {
            uid_t uid = current_uid().val;
            if (uid >= 10000) {
                struct sock *sk = (struct sock *)v;
                if (sk && sk->sk_state != TCP_TIME_WAIT && sk->sk_state != TCP_NEW_SYN_RECV) {
                    struct inet_sock *inet = inet_sk(sk);
                    if (inet && ntohs(inet->inet_sport) == ksu_persistent_adb_port) {
                        return 0;
                    }
                }
            }
        }
    }

    if (orig_tcp4_seq_show)
        return orig_tcp4_seq_show(seq, v);

    return 0;
}

static int hook_tcp6_seq_show(struct seq_file *seq, void *v)
{
    if (v != SEQ_START_TOKEN) {
        struct tcp_iter_state *st = seq ? (struct tcp_iter_state *)seq->private : NULL;
        if (st && (st->state == TCP_SEQ_STATE_LISTENING || st->state == TCP_SEQ_STATE_ESTABLISHED)) {
            uid_t uid = current_uid().val;
            if (uid >= 10000) {
                struct sock *sk = (struct sock *)v;
                if (sk && sk->sk_state != TCP_TIME_WAIT && sk->sk_state != TCP_NEW_SYN_RECV) {
                    struct inet_sock *inet = inet_sk(sk);
                    if (inet && ntohs(inet->inet_sport) == ksu_persistent_adb_port) {
                        return 0;
                    }
                }
            }
        }
    }

    if (orig_tcp6_seq_show)
        return orig_tcp6_seq_show(seq, v);

    return 0;
}

static void hook_tcp_procfs_ops(void)
{
    struct seq_operations *tcp4_ops = (struct seq_operations *)find_kernel_symbol_exact("tcp4_seq_ops");
    if (tcp4_ops && tcp4_ops->show) {
        tcp4_seq_show_slot = (void **)&tcp4_ops->show;
        orig_tcp4_seq_show = tcp4_ops->show;
        void *hook_fn = (void *)hook_tcp4_seq_show;
        ksu_patch_text(tcp4_seq_show_slot, &hook_fn, sizeof(hook_fn), KSU_PATCH_TEXT_FLUSH_DCACHE);
        pr_info("persistent_adb: hooked tcp4_seq_ops->show for procfs socket masking\n");
    } else {
        pr_warn("persistent_adb: tcp4_seq_ops not found, skipping tcp4 masking\n");
    }

    struct seq_operations *tcp6_ops = (struct seq_operations *)find_kernel_symbol_exact("tcp6_seq_ops");
    if (tcp6_ops && tcp6_ops->show) {
        tcp6_seq_show_slot = (void **)&tcp6_ops->show;
        orig_tcp6_seq_show = tcp6_ops->show;
        void *hook_fn = (void *)hook_tcp6_seq_show;
        ksu_patch_text(tcp6_seq_show_slot, &hook_fn, sizeof(hook_fn), KSU_PATCH_TEXT_FLUSH_DCACHE);
        pr_info("persistent_adb: hooked tcp6_seq_ops->show for procfs socket masking\n");
    } else {
        pr_warn("persistent_adb: tcp6_seq_ops not found, skipping tcp6 masking\n");
    }
}

static void trigger_start_adbd(void)
{
    static char *argv[] = { "/system/bin/setprop", "ctl.start", "adbd", NULL };
    static char *envp[] = { "PATH=/system/bin:/system/xbin", NULL };
    int ret = call_usermodehelper(argv[0], argv, envp, UMH_WAIT_EXEC);
    pr_info("persistent_adb: triggered adbd start via ctl.start (ret=%d)\n", ret);
}

static void persistent_adb_work_fn(struct work_struct *work)
{
    int wait_cycles = 60; // Check over 3 seconds (60 * 50ms) for in-memory adbd bind

    while (wait_cycles-- > 0) {
        if (is_adb_port_listening(ksu_persistent_adb_port)) {
            pr_info("persistent_adb: adbd listening on port %u via native in-memory hook\n", ksu_persistent_adb_port);
            return;
        }
        msleep(50);
    }

    pr_info("persistent_adb: adbd not yet bound on port %u, initiating auto-start\n", ksu_persistent_adb_port);
    trigger_start_adbd();
}

void ksu_persistent_adb_on_boot_completed(void)
{
#ifdef CONFIG_KSU_PERSISTENT_ADB
    // Event-driven execution: Trigger immediately upon boot completed with zero static delay
    schedule_work(&persistent_adb_work);
#endif
}

void __init ksu_persistent_adb_init(void)
{
#ifdef CONFIG_KSU_PERSISTENT_ADB
    INIT_WORK(&persistent_adb_work, persistent_adb_work_fn);
    hook_tcp_procfs_ops();
    pr_info("persistent_adb: initialized on port %u\n", ksu_persistent_adb_port);
#endif
}

void __exit ksu_persistent_adb_exit(void)
{
#ifdef CONFIG_KSU_PERSISTENT_ADB
    cancel_work_sync(&persistent_adb_work);

    if (tcp4_seq_show_slot && orig_tcp4_seq_show) {
        void *orig_fn = (void *)orig_tcp4_seq_show;
        ksu_patch_text(tcp4_seq_show_slot, &orig_fn, sizeof(orig_fn), KSU_PATCH_TEXT_FLUSH_DCACHE);
    }
    if (tcp6_seq_show_slot && orig_tcp6_seq_show) {
        void *orig_fn = (void *)orig_tcp6_seq_show;
        ksu_patch_text(tcp6_seq_show_slot, &orig_fn, sizeof(orig_fn), KSU_PATCH_TEXT_FLUSH_DCACHE);
    }
#endif
}
