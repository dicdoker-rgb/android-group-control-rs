/*
 * /proc/mounts filter via kretprobe on show_vfsmnt/show_mountinfo
 * v4: Only filters for container processes, NOT for Docker runtime.
 *     Uses dual check: mount namespace + process name exclusion.
 *     Docker/runc processes always see unfiltered mounts.
 */
#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/kprobes.h>
#include <linux/seq_file.h>
#include <linux/string.h>
#include <linux/nsproxy.h>
#include <linux/mnt_namespace.h>
#include <linux/sched.h>

MODULE_LICENSE("GPL");
MODULE_AUTHOR("groupcontrol");
MODULE_DESCRIPTION("Filter overlay/docker from /proc/mounts for containers only v4");

static struct mnt_namespace *init_mnt_ns;

struct mount_filter_data {
    struct seq_file *m;
    size_t old_count;
    int skip;
};

/* Check if this process should be exempt from filtering */
static inline int should_skip_filter(void)
{
    const char *comm = current->comm;

    /* Host processes (init mount namespace) - never filter */
    if (!current->nsproxy)
        return 1;
    if (current->nsproxy->mnt_ns == init_mnt_ns)
        return 1;

    /* Docker/container runtime processes - never filter even in new ns */
    if (strncmp(comm, "runc", 4) == 0)
        return 1;
    if (strstr(comm, "docker") != NULL)
        return 1;
    if (strstr(comm, "containerd") != NULL)
        return 1;
    if (strncmp(comm, "exe", 3) == 0)  /* runc uses "exe" as comm sometimes */
        return 1;

    return 0;
}

static int entry_handler(struct kretprobe_instance *ri, struct pt_regs *regs)
{
    struct mount_filter_data *data = (struct mount_filter_data *)ri->data;
    struct seq_file *m = (struct seq_file *)regs->regs[0];

    if (should_skip_filter()) {
        data->skip = 1;
        return 0;
    }

    data->skip = 0;
    data->m = m;
    data->old_count = m ? m->count : 0;
    return 0;
}

static int ret_handler(struct kretprobe_instance *ri, struct pt_regs *regs)
{
    struct mount_filter_data *data = (struct mount_filter_data *)ri->data;
    struct seq_file *m;
    size_t old_count, new_count, new_len;
    char *new_data;

    if (data->skip)
        return 0;

    m = data->m;
    old_count = data->old_count;

    if (!m || !m->buf)
        return 0;

    new_count = m->count;
    if (new_count <= old_count)
        return 0;

    new_data = m->buf + old_count;
    new_len = new_count - old_count;

    if (strnstr(new_data, "overlay", new_len) ||
        strnstr(new_data, "docker", new_len) ||
        strnstr(new_data, "magisk", new_len) ||
        strnstr(new_data, "lxcfs", new_len) ||
        strnstr(new_data, "containers", new_len) ||
        strnstr(new_data, "mmcblk", new_len)) {
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

    init_mnt_ns = current->nsproxy->mnt_ns;
    pr_info("mounts_filter_v4: init mnt_ns=%px\n", init_mnt_ns);

    krp_vfsmnt.kp.symbol_name = "show_vfsmnt";
    ret = register_kretprobe(&krp_vfsmnt);
    if (ret < 0) {
        pr_warn("mounts_filter_v4: show_vfsmnt failed: %d\n", ret);
    } else {
        vfsmnt_registered = 1;
        pr_info("mounts_filter_v4: hooked show_vfsmnt\n");
    }

    krp_mountinfo.kp.symbol_name = "show_mountinfo";
    ret = register_kretprobe(&krp_mountinfo);
    if (ret < 0) {
        pr_warn("mounts_filter_v4: show_mountinfo failed: %d\n", ret);
    } else {
        mountinfo_registered = 1;
        pr_info("mounts_filter_v4: hooked show_mountinfo\n");
    }

    if (!vfsmnt_registered && !mountinfo_registered) {
        pr_err("mounts_filter_v4: both hooks failed\n");
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
    pr_info("mounts_filter_v4: unloaded\n");
}

module_init(filter_init);
module_exit(filter_exit);
