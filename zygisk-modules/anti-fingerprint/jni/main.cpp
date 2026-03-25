/**
 * Anti-Fingerprint Zygisk Module
 *
 * 杀器一：Canvas/WebGL 指纹投毒 — Hook glReadPixels，随机篡改像素破坏 GPU 指纹
 * 杀器二：/proc 文件重定向 — Hook openat，将敏感文件读取重定向到伪造文件
 *
 * 基于 Dobby inline hook 框架
 */

#include <cstdlib>
#include <cstring>
#include <cstdio>
#include <ctime>
#include <unistd.h>
#include <fcntl.h>
#include <dlfcn.h>
#include <sys/types.h>
#include <android/log.h>

// Dobby hook framework
#include "dobby.h"

#define LOG_TAG "AntiFingerprint"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGW(...) __android_log_print(ANDROID_LOG_WARN, LOG_TAG, __VA_ARGS__)

// ============================================================================
// 目标进程过滤
// ============================================================================

static bool g_is_target_process = false;

// 需要保护的包名列表
static const char* TARGET_PACKAGES[] = {
    "com.ss.android.ugc.aweme",     // 抖音
    "com.zhiliaoapp.musically",     // TikTok
    "com.ss.android.ugc.trill",     // TikTok Lite
    nullptr
};

static bool is_target_package(const char* process_name) {
    if (!process_name) return false;
    for (int i = 0; TARGET_PACKAGES[i]; i++) {
        if (strstr(process_name, TARGET_PACKAGES[i])) {
            return true;
        }
    }
    return false;
}

static void check_process() {
    char cmdline[256] = {0};
    FILE* f = fopen("/proc/self/cmdline", "r");
    if (f) {
        fread(cmdline, 1, sizeof(cmdline) - 1, f);
        fclose(f);
    }
    g_is_target_process = is_target_package(cmdline);
    if (g_is_target_process) {
        LOGI("Target process detected: %s", cmdline);
    }
}

// ============================================================================
// 杀器一：Canvas/WebGL 指纹投毒
// ============================================================================

// glReadPixels 原型
// void glReadPixels(GLint x, GLint y, GLsizei width, GLsizei height,
//                   GLenum format, GLenum type, void* pixels);
typedef void (*glReadPixels_t)(int x, int y, int width, int height,
                                unsigned int format, unsigned int type, void* pixels);

static glReadPixels_t orig_glReadPixels = nullptr;

// OpenGL ES 常量
#define GL_RGBA             0x1908
#define GL_RGB              0x1907
#define GL_UNSIGNED_BYTE    0x1401

// 投毒参数
#define POISON_RATIO        0.01f   // 1% 的像素被投毒
#define POISON_MAX_OFFSET   2       // RGB 偏移量 [-2, +2]

static void hook_glReadPixels(int x, int y, int width, int height,
                               unsigned int format, unsigned int type, void* pixels) {
    // 1. 先执行原函数，让 GPU 完成真实渲染
    orig_glReadPixels(x, y, width, height, format, type, pixels);

    // 2. 只对目标进程投毒
    if (!g_is_target_process || !pixels) return;

    // 3. 只处理 UNSIGNED_BYTE 格式（最常用）
    if (type != GL_UNSIGNED_BYTE) return;

    // 4. 计算像素参数
    int channels = 0;
    if (format == GL_RGBA) channels = 4;
    else if (format == GL_RGB) channels = 3;
    else return;

    int total_pixels = width * height;
    if (total_pixels <= 0 || total_pixels > 16777216) return; // 安全限制：最大 4096x4096

    unsigned char* data = (unsigned char*)pixels;
    int poison_count = (int)(total_pixels * POISON_RATIO);
    if (poison_count < 1) poison_count = 1;

    // 5. 投毒：随机选像素，随机修改 RGB 通道
    for (int i = 0; i < poison_count; i++) {
        int pixel_idx = rand() % total_pixels;
        int byte_offset = pixel_idx * channels;
        int channel = rand() % 3; // 只改 R/G/B，不改 A

        int offset = (rand() % (POISON_MAX_OFFSET * 2 + 1)) - POISON_MAX_OFFSET;
        int new_val = (int)data[byte_offset + channel] + offset;

        // 防止溢出
        if (new_val < 0) new_val = 0;
        if (new_val > 255) new_val = 255;

        data[byte_offset + channel] = (unsigned char)new_val;
    }
}

// ============================================================================
// 杀器二：/proc 文件重定向
// ============================================================================

// openat 原型
// int openat(int dirfd, const char *pathname, int flags, ...);
typedef int (*openat_t)(int dirfd, const char* pathname, int flags, ...);
static openat_t orig_openat = nullptr;

// 需要重定向的文件映射
struct FileRedirect {
    const char* original;       // 原始路径
    const char* fake;           // 伪造文件路径
};

static const FileRedirect REDIRECTS[] = {
    { "/proc/self/mountinfo",   "/data/local/tmp/fake_mountinfo" },
    { "/proc/mounts",           "/data/local/tmp/fake_mounts" },
    { "/proc/self/mounts",      "/data/local/tmp/fake_mounts" },
    { "/proc/self/cgroup",      "/data/local/tmp/fake_cgroup" },
    { "/proc/1/cgroup",         "/data/local/tmp/fake_cgroup" },
    { nullptr, nullptr }
};

// 检查路径是否需要重定向
static const char* get_redirect_path(const char* pathname) {
    if (!pathname) return nullptr;
    for (int i = 0; REDIRECTS[i].original; i++) {
        if (strcmp(pathname, REDIRECTS[i].original) == 0) {
            // 检查伪造文件是否存在
            if (access(REDIRECTS[i].fake, R_OK) == 0) {
                return REDIRECTS[i].fake;
            }
        }
    }
    return nullptr;
}

static int hook_openat(int dirfd, const char* pathname, int flags, ...) {
    // 只对目标进程生效
    if (g_is_target_process && pathname) {
        const char* redirect = get_redirect_path(pathname);
        if (redirect) {
            LOGI("IO redirect: %s -> %s", pathname, redirect);
            return orig_openat(dirfd, redirect, flags);
        }

        // 额外检查：拦截对 /.dockerenv 的访问
        if (strcmp(pathname, "/.dockerenv") == 0) {
            // 返回 ENOENT（文件不存在）
            errno = 2; // ENOENT
            return -1;
        }
    }

    // mode 参数处理（当 flags 包含 O_CREAT 时需要）
    if (flags & O_CREAT) {
        va_list args;
        va_start(args, flags);
        mode_t mode = va_arg(args, int);
        va_end(args);
        return orig_openat(dirfd, pathname, flags, mode);
    }

    return orig_openat(dirfd, pathname, flags);
}

// ============================================================================
// 额外防护：/proc/self/maps 过滤 Magisk/Xposed 痕迹
// ============================================================================

// read 原型
typedef ssize_t (*read_t)(int fd, void* buf, size_t count);
static read_t orig_read = nullptr;

// 记录哪些 fd 是敏感文件
#define MAX_TRACKED_FDS 64
static int tracked_fds[MAX_TRACKED_FDS] = {0};
static int tracked_fd_count = 0;

static bool is_tracked_fd(int fd) {
    for (int i = 0; i < tracked_fd_count; i++) {
        if (tracked_fds[i] == fd) return true;
    }
    return false;
}

// 从 read 返回的缓冲区中过滤敏感行
static ssize_t filter_sensitive_lines(char* buf, ssize_t len) {
    if (len <= 0) return len;

    // 敏感关键字
    static const char* SENSITIVE[] = {
        "magisk", "zygisk", "lsposed", "xposed", "shamiko",
        "riru", "edxposed", "frida", "substrate",
        nullptr
    };

    char* write_pos = buf;
    char* read_pos = buf;
    char* end = buf + len;

    while (read_pos < end) {
        // 找行尾
        char* line_end = (char*)memchr(read_pos, '\n', end - read_pos);
        if (!line_end) line_end = end;
        else line_end++; // 包含换行符

        int line_len = line_end - read_pos;

        // 检查是否包含敏感关键字
        bool sensitive = false;
        for (int i = 0; SENSITIVE[i]; i++) {
            // 临时 null-terminate 来用 strcasestr
            char saved = *line_end;
            if (line_end < end) *line_end = '\0';
            if (strcasestr(read_pos, SENSITIVE[i])) {
                sensitive = true;
            }
            if (line_end < end) *line_end = saved;
            if (sensitive) break;
        }

        if (!sensitive) {
            if (write_pos != read_pos) {
                memmove(write_pos, read_pos, line_len);
            }
            write_pos += line_len;
        }

        read_pos = line_end;
    }

    return write_pos - buf;
}

// ============================================================================
// 模块初始化
// ============================================================================

static void install_hooks() {
    srand((unsigned int)time(nullptr) ^ (unsigned int)getpid());

    check_process();

    // Hook glReadPixels（杀器一：Canvas 投毒）
    void* libgles = dlopen("libGLESv2.so", RTLD_NOW);
    if (libgles) {
        void* sym = dlsym(libgles, "glReadPixels");
        if (sym) {
            DobbyHook(sym, (void*)hook_glReadPixels, (void**)&orig_glReadPixels);
            LOGI("glReadPixels hooked at %p", sym);
        }
    }

    // Hook openat（杀器二：IO 重定向）
    void* libc = dlopen("libc.so", RTLD_NOW);
    if (libc) {
        void* sym_openat = dlsym(libc, "openat");
        if (sym_openat) {
            DobbyHook(sym_openat, (void*)hook_openat, (void**)&orig_openat);
            LOGI("openat hooked at %p", sym_openat);
        }
    }

    LOGI("Anti-fingerprint hooks installed (target=%s)",
         g_is_target_process ? "YES" : "NO");
}

// ============================================================================
// Zygisk 入口
// ============================================================================

// Zygisk API
#include "zygisk.hpp"

class AntiFingerprint : public zygisk::ModuleBase {
public:
    void onLoad(zygisk::Api* api, JNIEnv* env) override {
        this->api = api;
        this->env = env;
    }

    void preAppSpecialize(zygisk::AppSpecializeArgs* args) override {
        // 获取包名
        const char* process = env->GetStringUTFChars(args->nice_name, nullptr);
        if (process) {
            is_target = is_target_package(process);
            env->ReleaseStringUTFChars(args->nice_name, process);
        }

        if (!is_target) {
            // 非目标进程，卸载模块节省资源
            api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY);
            return;
        }
    }

    void postAppSpecialize(const zygisk::AppSpecializeArgs* args) override {
        if (!is_target) return;

        g_is_target_process = true;
        install_hooks();
    }

private:
    zygisk::Api* api = nullptr;
    JNIEnv* env = nullptr;
    bool is_target = false;
};

REGISTER_ZYGISK_MODULE(AntiFingerprint)
