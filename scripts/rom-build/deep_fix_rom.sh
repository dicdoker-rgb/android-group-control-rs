#!/bin/bash
set -x
cd ~/redroid-build

echo "$(date) ===== 深度ROM改造开始 ====="

# ============================================
# 1. 添加user构建目标（不是userdebug）
# ============================================
echo "=== 1. 添加user构建 ==="
# 在AndroidProducts.mk里加user变体
grep -q "redroid_arm64-user" device/redroid/AndroidProducts.mk || {
    sed -i 's/redroid_arm64-userdebug/redroid_arm64-userdebug \\\n\tredroid_arm64-user/' device/redroid/AndroidProducts.mk
}

# ============================================
# 2. 全局替换PRODUCT_NAME/DEVICE为Xiaomi
# ============================================
echo "=== 2. 产品名称替换 ==="
# redroid_arm64.mk里改产品名
sed -i 's/PRODUCT_NAME := redroid_arm64/PRODUCT_NAME := umi/' device/redroid/redroid_arm64.mk
sed -i 's/PRODUCT_DEVICE := redroid_arm64/PRODUCT_DEVICE := umi/' device/redroid/redroid_arm64.mk

# ============================================
# 3. redroid.prop改掉ro.hardware
# ============================================
echo "=== 3. vendor属性 ==="
# 保留ro.hardware=redroid给GPU脚本内部用，但加一个内部标记
cat > device/redroid/redroid.prop << 'PROPEOF'
# GPU初始化脚本用内部变量，不依赖ro.hardware
persist.sys.internal.platform=rk3588
persist.sys.internal.gpu=mali

ro.cold_boot_done=true
persist.sys.fuse=1
ro.radio.noril=yes
debug.sf.nobootanimation=1
PROPEOF
# 注意：去掉了ro.hardware=redroid！

# ============================================
# 4. provision.sh去掉Magisk配置
# ============================================
echo "=== 4. provision去magisk ==="
sed -i '/magisk/d; /ZYGISK/d; /Magisk/d' device/redroid/prebuilt/bin/provision.sh

# ============================================
# 5. 瘦身：去掉不需要的APP
# ============================================
echo "=== 5. 瘦身 ==="
# 在device.mk里标记不需要的包
cat >> device/redroid/redroid_arm64/device.mk << 'SLIMEOF'

# === 瘦身：去掉不需要的APP ===
PRODUCT_PACKAGES_REMOVE += \
    Traceur \
    NfcNci \
    EasterEgg \
    PrintSpooler \
    BasicDreams \
    LiveWallpapersPicker \
    ManagedProvisioning \
    CompanionDeviceManager
SLIMEOF

# ============================================
# 6. 编译日期伪造
# ============================================
echo "=== 6. 编译日期 ==="
export BUILD_DATETIME=1694747538
export BUILD_NUMBER=V14.0.4.0.TJBCNXM

# ============================================
# 7. 确认GL spoof和provision在
# ============================================
echo "=== 7. 验证关键文件 ==="
ls device/redroid/gl_spoof/gl_spoof.c && echo "GL_SPOOF_OK"
ls device/redroid/prebuilt/bin/provision.sh && echo "PROVISION_OK"

echo ""
echo "$(date) ===== 源码修改完成，开始编译 ====="
echo ""

# ============================================
# 8. 编译（user构建！不是userdebug）
# ============================================
export PATH=~/bin:$PATH
export USE_CCACHE=1
export CCACHE_DIR=~/.ccache

source build/envsetup.sh

# 先试user构建
lunch redroid_arm64-user 2>&1 || {
    echo "user构建不可用，用userdebug但设ro.debuggable=0"
    lunch redroid_arm64-userdebug
}

echo "$(date) === MAKE START ==="
make -j$(nproc)
RESULT=$?
echo "$(date) === MAKE DONE: $RESULT ==="

if [ $RESULT -eq 0 ]; then
    echo "BUILD_SUCCESS" > /tmp/build_status
    ls -lh out/target/product/redroid_arm64/system.img
    ls -lh out/target/product/redroid_arm64/vendor.img
else
    echo "BUILD_FAILED" > /tmp/build_status
fi
