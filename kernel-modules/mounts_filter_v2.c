/*
 * /proc/mounts filter via kretprobe on show_vfsmnt
 * Uses kretprobe per-instance data to correctly pass seq_file pointer
 * from entry to return handler.
 *
 * Bug fix: Original kprobe version read regs->regs[0] in post_handler,
 * which on ARM64 is the return value (0), not the original seq_file arg.
 */
#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/kprobes.h>
#include <linux/seq_file.h>
#include <linux/string.h>

MODULE_LICENSE("GPL");
MODULE_AUTHOR("groupcontrol");
MODULE_DESCRIPTION("Filter overlay/docker from /proc/mounts via kretprobe");

/* Per-instance data saved between entry and return */
struct mount_filter_data {
    struct seq_file *m;
    size_t old_count;
};

/* Entry handler: save seq_file pointer and current position */
static int entry_handler(struct kretprobe_instance *ri, struct pt_regs *regs)
{
    struct mount_filter_data *data = (struct mount_filter_data *)ri->data;
    struct seq_file *m = (struct seq_file *)regs->regs[0];

    data->m = m;
    data->old_count = m ? m->count : 0;

    return 0;
}

/* Return handler: check newly written data, truncate if overlay/docker */
static int ret_handler(struct kretprobe_instance *ri, struct pt_regs *regs)
{
    struct mount_filter_data *data = (struct mount_filter_data *)ri->data;
    struct seq_file *m = data->m;
    size_t old_count = data->old_count;
    size_t new_count;
    char *new_data;
    size_t new_len;

    if (!m || !m->buf)
        return 0;

    new_count = m->count;
    if (new_count <= old_count)
        return 0;

    new_data = m->buf + old_count;
    new_len = new_count - old_count;

    /* Filter mount entries containing these keywords */
    if (strnstr(new_data, "overlay", new_len) ||
        strnstr(new_data, "docker", new_len) ||
        strnstr(new_data, "magisk", new_len) ||
        strnstr(new_data, "lxcfs", new_len) ||
        strnstr(new_data, "containers", new_len) ||
        strnstr(new_data, "mmcblk", new_len)) {
        /* Undo: truncate seq_file back to old position */
        m->count = old_count;
    }

    return 0;
}

static struct kretprobe krp_vfsmnt = {
    .handler = ret_handler,
    .entry_handler = entry_handler,
    .data_size = sizeof(struct mount_filter_data),
    .maxactive = 64,
};

static struct kretprobe krp_mountinfo = {
    .handler = ret_handler,
    .entry_handler = entry_handler,
    .data_size = sizeof(struct mount_filter_data),
    .maxactive = 64,
};

static int vfsmnt_registered = 0;
static int mountinfo_registered = 0;

static int __init filter_init(void)
{
    int ret;

    /* Hook show_vfsmnt (used by /proc/mounts) */
    krp_vfsmnt.kp.symbol_name = "show_vfsmnt";
    ret = register_kretprobe(&krp_vfsmnt);
    if (ret < 0) {
        pr_warn("mounts_filter_v2: show_vfsmnt kretprobe failed: %d\n", ret);
    } else {
        vfsmnt_registered = 1;
        pr_info("mounts_filter_v2: hooked show_vfsmnt\n");
    }

    /* Hook show_mountinfo (used by /proc/self/mountinfo) */
    krp_mountinfo.kp.symbol_name = "show_mountinfo";
    ret = register_kretprobe(&krp_mountinfo);
    if (ret < 0) {
        pr_warn("mounts_filter_v2: show_mountinfo kretprobe failed: %d\n", ret);
    } else {
        mountinfo_registered = 1;
        pr_info("mounts_filter_v2: hooked show_mountinfo\n");
    }

    if (!vfsmnt_registered && !mountinfo_registered) {
        pr_err("mounts_filter_v2: both hooks failed, giving up\n");
        return -ENOENT;
    }

    return 0;
}

static void __exit filter_exit(void)
{
    if (vfsmnt_registered)
        unregister_kretprobe(&krp_vfsmnt);
    if (mountinfo_registered)
        unregister_kretprobe(&krp_mountinfo);
    pr_info("mounts_filter_v2: unloaded\n");
}

module_init(filter_init);
module_exit(filter_exit);
