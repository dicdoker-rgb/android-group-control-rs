/*
 * /proc/self/maps filter: replace "mali0" with "kgsl" in output
 * Hooks show_map_vma (renders each line of /proc/<pid>/maps)
 * Only applies to non-host mount namespaces (container processes)
 */
#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/kprobes.h>
#include <linux/seq_file.h>
#include <linux/string.h>
#include <linux/nsproxy.h>
#include <linux/mnt_namespace.h>

MODULE_LICENSE("GPL");
MODULE_AUTHOR("groupcontrol");
MODULE_DESCRIPTION("Filter mali/rk3588 from /proc/pid/maps for containers");

static struct mnt_namespace *init_mnt_ns;

struct maps_filter_data {
    struct seq_file *m;
    size_t old_count;
    int skip;
};

static inline int is_host_process(void)
{
    if (!current->nsproxy) return 1;
    return current->nsproxy->mnt_ns == init_mnt_ns;
}

static int entry_handler(struct kretprobe_instance *ri, struct pt_regs *regs)
{
    struct maps_filter_data *data = (struct maps_filter_data *)ri->data;
    struct seq_file *m = (struct seq_file *)regs->regs[0];

    if (is_host_process()) {
        data->skip = 1;
        return 0;
    }
    data->skip = 0;
    data->m = m;
    data->old_count = m ? m->count : 0;
    return 0;
}

/* Simple in-place replace within a buffer region */
static void replace_in_buf(char *buf, size_t len, const char *from, const char *to)
{
    size_t from_len = strlen(from);
    size_t to_len = strlen(to);
    char *p;

    /* Only handle same-length or shorter replacements to avoid buffer issues */
    if (to_len > from_len)
        return;

    while ((p = strnstr(buf, from, len)) != NULL) {
        memcpy(p, to, to_len);
        if (to_len < from_len) {
            /* Shift remaining data left */
            size_t tail = len - (p - buf) - from_len;
            memmove(p + to_len, p + from_len, tail);
            len -= (from_len - to_len);
        }
    }
}

static int ret_handler(struct kretprobe_instance *ri, struct pt_regs *regs)
{
    struct maps_filter_data *data = (struct maps_filter_data *)ri->data;
    struct seq_file *m;
    size_t old_count, new_count, new_len;
    char *new_data;

    if (data->skip)
        return 0;

    m = data->m;
    old_count = data->old_count;
    if (!m || !m->buf) return 0;

    new_count = m->count;
    if (new_count <= old_count) return 0;

    new_data = m->buf + old_count;
    new_len = new_count - old_count;

    /* Replace GPU device names */
    replace_in_buf(new_data, new_len, "/dev/mali0", "/dev/kgsl0");
    replace_in_buf(new_data, new_len, "mali0", "kgsl0");

    /* Replace Mali CSF kernel mappings */
    replace_in_buf(new_data, new_len, "/mali csf", "/gpu  csf");
    replace_in_buf(new_data, new_len, "/mali ", "/gpu  ");

    /* Replace rockchip references */
    replace_in_buf(new_data, new_len, "libGLES_mali", "libGLES_adre");
    replace_in_buf(new_data, new_len, "mali-dp_dri", "adreno_dri.");

    return 0;
}

static struct kretprobe krp_maps = {
    .handler = ret_handler,
    .entry_handler = entry_handler,
    .data_size = sizeof(struct maps_filter_data),
    .maxactive = 64,
};

static int maps_registered = 0;

static int __init filter_init(void)
{
    int ret;
    init_mnt_ns = current->nsproxy->mnt_ns;

    krp_maps.kp.symbol_name = "show_map_vma";
    ret = register_kretprobe(&krp_maps);
    if (ret < 0) {
        pr_err("maps_filter: show_map_vma hook failed: %d\n", ret);
        return ret;
    }
    maps_registered = 1;
    pr_info("maps_filter: hooked show_map_vma\n");
    return 0;
}

static void __exit filter_exit(void)
{
    if (maps_registered)
        unregister_kretprobe(&krp_maps);
    pr_info("maps_filter: unloaded\n");
}

module_init(filter_init);
module_exit(filter_exit);
