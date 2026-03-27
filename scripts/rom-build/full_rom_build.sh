#!/bin/bash
exec > /tmp/build.log 2>&1
set -x

echo "$(date) ===== 全自动ROM编译 ====="

# ============================================
# 阶段0: 环境安装 (5分钟)
# ============================================
echo "$(date) === 阶段0: 环境 ==="
apt-get update -qq
apt-get install -y -qq openjdk-17-jdk-headless git git-lfs \
    bc bison build-essential ccache curl flex g++-multilib \
    gcc-multilib gnupg gperf libelf-dev libncurses5 \
    libssl-dev libxml2-utils lzop zip zlib1g-dev \
    python3 python3-pip rsync unzip wget screen \
    docker.io qemu-user-static binfmt-support

# repo
mkdir -p ~/bin
curl -s https://mirrors.ustc.edu.cn/aosp/git-repo > ~/bin/repo 2>/dev/null || \
    curl -s https://storage.googleapis.com/git-repo-downloads/repo > ~/bin/repo
chmod +x ~/bin/repo
export PATH=~/bin:$PATH

# git config
git config --global user.email "build@xiaomi.com"
git config --global user.name "android-build"
git config --global color.ui false

# Docker + QEMU for testing
systemctl start docker
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes 2>/dev/null

echo "$(date) === 环境安装完成 ==="

# ============================================
# 阶段1: AOSP源码同步 (30-60分钟)
# ============================================
echo "$(date) === 阶段1: repo sync ==="
export REPO_URL="https://mirrors.ustc.edu.cn/aosp/git-repo"
mkdir -p ~/redroid-build && cd ~/redroid-build

repo init -u https://mirrors.ustc.edu.cn/aosp/platform/manifest --depth=1 -b android-13.0.0_r74 --no-clone-bundle

# 多次重试sync
for attempt in 1 2 3 4 5; do
    echo "$(date) sync attempt $attempt"
    repo sync -c -j8 --no-tags --force-sync --no-clone-bundle
    if [ -f build/envsetup.sh ]; then
        echo "$(date) SYNC COMPLETE"
        echo "SYNC_DONE" > /tmp/build_status
        break
    fi
    sleep 10
done

if [ ! -f build/envsetup.sh ]; then
    echo "SYNC FAILED"
    echo "SYNC_FAILED" > /tmp/build_status
    exit 1
fi

# ============================================
# 阶段2: Redroid设备树 (2分钟)
# ============================================
echo "$(date) === 阶段2: 设备树 ==="
cd ~/redroid-build

# 从GitHub clone (这台服务器应该能访问)
git clone --depth=1 https://github.com/remote-android/device_redroid.git device/redroid 2>&1 || echo "device clone failed"
git clone --depth=1 https://github.com/remote-android/vendor_redroid.git vendor/redroid 2>&1 || echo "vendor clone failed"
git clone --depth=1 https://github.com/remote-android/device_redroid-prebuilts.git device/redroid-prebuilts 2>&1 || echo "prebuilts clone failed"

# ============================================
# 阶段3: 深度定制 (5分钟)
# ============================================
echo "$(date) === 阶段3: 定制 ==="

# --- 3a. 添加user构建目标 ---
grep -q "redroid_arm64-user" device/redroid/AndroidProducts.mk || {
    sed -i '/redroid_arm64-userdebug/a\\tredroid_arm64-user \\' device/redroid/AndroidProducts.mk
}

# --- 3b. 产品名改为Xiaomi ---
sed -i 's/PRODUCT_NAME := redroid_arm64/PRODUCT_NAME := umi/' device/redroid/redroid_arm64.mk
sed -i 's/PRODUCT_DEVICE := redroid_arm64/PRODUCT_DEVICE := umi/' device/redroid/redroid_arm64.mk

# --- 3c. device.mk全量伪装 ---
cat >> device/redroid/redroid_arm64/device.mk << 'DEVEOF'

# ===== 群控ROM定制 v6 =====
PRODUCT_BRAND := Xiaomi
PRODUCT_MODEL := M2001J2C
PRODUCT_DEVICE := umi
PRODUCT_MANUFACTURER := Xiaomi

PRODUCT_PROPERTY_OVERRIDES += \
    persist.sys.language=zh \
    persist.sys.country=CN \
    persist.sys.locale=zh-CN \
    persist.sys.timezone=Asia/Shanghai \
    ro.product.locale=zh-CN \
    ro.setupwizard.mode=DISABLED \
    ro.board.platform=kona \
    ro.hardware.chipname=sm8250 \
    ro.hardware.vulkan=adreno \
    ro.hardware.composer=drm \
    ro.build.display.id=V14.0.4.0.TJBCNXM \
    ro.build.version.incremental=V14.0.4.0.TJBCNXM \
    ro.build.host=build.xiaomi.com \
    ro.build.user=android-build \
    ro.build.flavor=umi-user \
    persist.service.adb.enable=0 \
    ro.adb.secure=1

BUILD_FINGERPRINT := Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys
BUILD_DESCRIPTION := umi-user 13 TKQ1.221114.001 V14.0.4.0.TJBCNXM release-keys

# 瘦身
PRODUCT_PACKAGES_REMOVE += \
    Traceur NfcNci EasterEgg PrintSpooler BasicDreams \
    LiveWallpapersPicker ManagedProvisioning CompanionDeviceManager

PRODUCT_PACKAGES += LatinIME
DEVEOF

# --- 3d. redroid.prop去掉ro.hardware=redroid ---
cat > device/redroid/redroid.prop << 'PROPEOF'
persist.sys.internal.platform=rk3588
persist.sys.internal.gpu=mali
ro.cold_boot_done=true
persist.sys.fuse=1
ro.radio.noril=yes
debug.sf.nobootanimation=1
PROPEOF

# --- 3e. GL spoof ---
mkdir -p device/redroid/gl_spoof
cat > device/redroid/gl_spoof/gl_spoof.c << 'GLEOF'
#define _GNU_SOURCE
#include <dlfcn.h>
#include <string.h>
#define GL_VENDOR 0x1F00
#define GL_RENDERER 0x1F01
#define GL_VERSION 0x1F02
#define EGL_VENDOR 0x3053
#define EGL_VERSION 0x3054
typedef const char* (*fn_t)(unsigned int);
typedef const char* (*efn_t)(void*, int);
static fn_t real_gl = NULL;
static efn_t real_egl = NULL;
typedef void (*glReadPixels_t)(int,int,int,int,unsigned int,unsigned int,void*);
static glReadPixels_t real_glReadPixels = NULL;
static unsigned int noise_seed = 0x12345678;
static unsigned int lcg_next(void) { noise_seed = noise_seed * 1103515245 + 12345; return noise_seed; }
const char* glGetString(unsigned int name) {
    if (!real_gl) real_gl = (fn_t)dlsym(RTLD_NEXT, "glGetString");
    switch(name) {
        case GL_VENDOR: return "Qualcomm";
        case GL_RENDERER: return "Adreno (TM) 650";
        case GL_VERSION: return "OpenGL ES 3.2 V@0502.0 (GIT@9deb42f, I3a5765dd96, 1609190895)";
        default: return real_gl ? real_gl(name) : "";
    }
}
const char* eglQueryString(void* dpy, int name) {
    if (!real_egl) real_egl = (efn_t)dlsym(RTLD_NEXT, "eglQueryString");
    switch(name) {
        case EGL_VENDOR: return "Qualcomm Inc.";
        case EGL_VERSION: return "1.5 Adreno";
        default: return real_egl ? real_egl(dpy, name) : "";
    }
}
void glReadPixels(int x, int y, int w, int h, unsigned int fmt, unsigned int type, void* pixels) {
    if (!real_glReadPixels) real_glReadPixels = (glReadPixels_t)dlsym(RTLD_NEXT, "glReadPixels");
    if (real_glReadPixels) real_glReadPixels(x, y, w, h, fmt, type, pixels);
    if (!pixels || w <= 0 || h <= 0) return;
    noise_seed ^= (unsigned int)(w * 31337 + h * 7919 + x + y);
    unsigned char* p = (unsigned char*)pixels;
    int total = w * h * 4;
    int i;
    for (i = 0; i < total; i += 97*4) {
        int v = p[i] + (int)(lcg_next() % 5) - 2;
        p[i] = v < 0 ? 0 : (v > 255 ? 255 : v);
    }
}
GLEOF

cat > device/redroid/gl_spoof/Android.mk << 'GLMK'
LOCAL_PATH := $(call my-dir)
include $(CLEAR_VARS)
LOCAL_MODULE := libgl_spoof
LOCAL_SRC_FILES := gl_spoof.c
LOCAL_SHARED_LIBRARIES := libdl
LOCAL_MODULE_TAGS := optional
LOCAL_MULTILIB := both
LOCAL_MODULE_CLASS := SHARED_LIBRARIES
include $(BUILD_SHARED_LIBRARY)
GLMK

grep -q "libgl_spoof" device/redroid/redroid_arm64/device.mk || \
    echo "PRODUCT_PACKAGES += libgl_spoof" >> device/redroid/redroid_arm64/device.mk

# --- 3f. 禁用OMX(hardware/redroid不存在) ---
sed -i 's|$(call inherit-product, hardware/redroid|#$(call inherit-product, hardware/redroid|g' device/redroid/redroid.mk

# --- 3g. BoardConfig修复 ---
sed -i 's|generic_arm64_ab|generic_arm64|g' device/redroid/redroid_arm64/BoardConfig.mk 2>/dev/null

# --- 3h. 编译日期伪造 ---
export BUILD_DATETIME=1694747538
export BUILD_NUMBER=V14.0.4.0.TJBCNXM

echo "$(date) === 定制完成 ==="

# ============================================
# 阶段4: 编译 (30-60分钟)
# ============================================
echo "$(date) === 阶段4: 编译 ==="
cd ~/redroid-build
export USE_CCACHE=1
export CCACHE_DIR=~/.ccache

source build/envsetup.sh

# 尝试user构建
lunch redroid_arm64-user 2>&1 && {
    echo "$(date) === USER构建 ==="
} || {
    echo "$(date) === user不可用,用userdebug ==="
    lunch redroid_arm64-userdebug
}

make -j$(nproc)
RESULT=$?
echo "$(date) === 编译结果: $RESULT ==="

if [ $RESULT -eq 0 ]; then
    echo "BUILD_SUCCESS" > /tmp/build_status
    ls -lh out/target/product/*/system.img out/target/product/*/vendor.img 2>/dev/null
else
    echo "BUILD_FAILED" > /tmp/build_status
fi

echo "$(date) ===== 完成 ====="
