#!/bin/bash
set -x
cd ~/redroid-build

echo "$(date) === 补全遗漏修改 ==="

# ============================================
# 5. 不编Magisk — 找到Magisk编译入口并禁用
# ============================================
echo "=== 5. 禁用Magisk ==="
# Magisk可能在vendor_ext或device树里
grep -rn "magisk\|MAGISK" device/redroid/ vendor/redroid/ 2>/dev/null | grep -v "Binary\|\.git\|provision\|#" | head -10
# 如果有magisk相关的mk include，注释掉
find device/redroid vendor/redroid -name "*.mk" -exec sed -i 's/.*magisk.*/#&  # DISABLED/' {} \; 2>/dev/null
# 如果有magisk相关的PRODUCT_COPY_FILES
find device/redroid -name "*.mk" -exec sed -i '/magisk/Id' {} \; 2>/dev/null

# ============================================
# 6. Lineage痕迹源码级清除
# ============================================
echo "=== 6. Lineage清除 ==="
# 在所有mk/prop文件里删lineage行
find device/redroid -name "*.prop" -exec sed -i '/ro\.lineage/d; /ro\.modversion/d; /UNOFFICIAL/d' {} \;
find device/redroid -name "*.mk" -exec sed -i '/lineage/Id' {} \; 2>/dev/null

# ============================================  
# 7. 所有分区build.prop — 在device.mk里覆盖所有分区
# ============================================
echo "=== 7. 全分区属性 ==="
cat >> device/redroid/redroid_arm64/device.mk << 'ALLPARTEOF'

# 全分区统一Xiaomi (覆盖odm/system_ext/vendor_dlkm)
PRODUCT_PROPERTY_OVERRIDES += \
    ro.product.odm.brand=Xiaomi \
    ro.product.odm.device=umi \
    ro.product.odm.manufacturer=Xiaomi \
    ro.product.odm.model=M2001J2C \
    ro.product.odm.name=umi \
    ro.product.system_ext.brand=Xiaomi \
    ro.product.system_ext.device=umi \
    ro.product.system_ext.manufacturer=Xiaomi \
    ro.product.system_ext.model=M2001J2C \
    ro.product.system_ext.name=umi \
    ro.product.vendor_dlkm.brand=Xiaomi \
    ro.product.vendor_dlkm.device=umi \
    ro.product.vendor_dlkm.manufacturer=Xiaomi \
    ro.product.vendor_dlkm.model=M2001J2C \
    ro.product.vendor_dlkm.name=umi \
    ro.boot.verifiedbootstate=green \
    ro.boot.vbmeta.device_state=locked \
    ro.boot.selinux=enforcing \
    ro.debuggable=0 \
    ro.secure=1
ALLPARTEOF

# ============================================
# 8+9. gralloc/hwcomposer/rc文件重命名
# ============================================
echo "=== 8+9. 文件重命名 ==="
# 在vendor.mk里改HAL模块名
find vendor/redroid -name "*.bp" -exec sed -i 's/gralloc\.redroid/gralloc.default/g; s/hwcomposer\.redroid/hwcomposer.default/g' {} \; 2>/dev/null
find vendor/redroid -name "*.mk" -exec sed -i 's/gralloc\.redroid/gralloc.default/g; s/hwcomposer\.redroid/hwcomposer.default/g' {} \; 2>/dev/null

# RC文件重命名
if [ -f device/redroid/init.redroid.rc ]; then
    cp device/redroid/init.redroid.rc device/redroid/init.device.rc
    sed -i 's/redroid/device/g' device/redroid/init.device.rc
    # 更新引用
    find device/redroid -name "*.mk" -exec sed -i 's/init\.redroid\.rc/init.device.rc/g' {} \;
fi

# ============================================
# 10. SELinux permissive内核参数
# ============================================
echo "=== 10. SELinux ==="
grep -q "androidboot.selinux" device/redroid/redroid_arm64/BoardConfig.mk || \
    echo 'BOARD_KERNEL_CMDLINE += androidboot.selinux=permissive' >> device/redroid/redroid_arm64/BoardConfig.mk

# ============================================
# 11. app_process64 wrapper — 编进ROM
# ============================================
echo "=== 11. app_process64 wrapper ==="
# 创建wrapper脚本
mkdir -p device/redroid/prebuilt/bin
cat > device/redroid/prebuilt/bin/app_process64_wrapper << 'WRAPEOF'
#!/system/bin/sh
export LD_PRELOAD=/system/lib64/libgl_spoof.so
exec /system/bin/app_process64_real "$@"
WRAPEOF
chmod 755 device/redroid/prebuilt/bin/app_process64_wrapper

# 添加到编译（重命名原始app_process64并替换为wrapper）
# 这个需要在post-install脚本里做，编译时直接做会冲突
# 改用init.rc方式：在late_start时设置
cat > device/redroid/prebuilt/init.gl_spoof.rc << 'GLRCEOF'
# GL spoof activation - rename app_process64 and set wrapper
on post-fs
    exec u:r:init:s0 -- /system/bin/sh -c "if [ ! -f /system/bin/app_process64_real ]; then cp /system/bin/app_process64 /system/bin/app_process64_real; cp /system/bin/app_process64_wrapper /system/bin/app_process64; chmod 755 /system/bin/app_process64 /system/bin/app_process64_real; fi"
GLRCEOF

cat >> device/redroid/redroid_arm64/device.mk << 'WRAPERMK'

# GL spoof wrapper
PRODUCT_COPY_FILES += \
    device/redroid/prebuilt/bin/app_process64_wrapper:$(TARGET_COPY_OUT_SYSTEM)/bin/app_process64_wrapper \
    device/redroid/prebuilt/init.gl_spoof.rc:$(TARGET_COPY_OUT_SYSTEM)/etc/init/init.gl_spoof.rc
WRAPERMK

# ============================================
# 12. provision.sh更新（去Magisk、加v5.3修复）
# ============================================
echo "=== 12. provision更新 ==="
# 从provision里删掉所有magisk相关
sed -i '/magisk/Id; /ZYGISK/Id; /Magisk/Id; /shamiko/Id' device/redroid/prebuilt/bin/provision.sh 2>/dev/null

# 加入残留属性清理
cat >> device/redroid/prebuilt/bin/provision.sh << 'PROVEOF'

# === v6清理 ===
# 清理Docker启动参数暴露
for prop in ro.boot.redroid_gpu_mode ro.boot.redroid_gpu_node ro.boot.redroid_fps ro.boot.redroid_magisk ro.boot.redroid_width ro.boot.redroid_height; do
    setprop $prop "" 2>/dev/null
done
PROVEOF

# ============================================
# 13. incremental版本号统一
# ============================================
echo "=== 13. incremental ==="
# 确保所有地方的incremental都是V14.0.4.0.TJBCNXM
grep -rn "flysky" device/redroid/ 2>/dev/null | grep -v "Binary\|\.git\|#" | head -5

echo "$(date) === 补全完成 ==="

# ============================================
# 14. 深度瘦身
# ============================================
echo "=== 14. 深度瘦身 ==="
cat >> device/redroid/redroid_arm64/device.mk << 'SLIMEOF2'

# 深度瘦身 - 群控场景不需要的
PRODUCT_PACKAGES_REMOVE += \
    Traceur NfcNci EasterEgg PrintSpooler BasicDreams \
    LiveWallpapersPicker ManagedProvisioning CompanionDeviceManager \
    Camera2 Gallery2 Music SoundRecorder Calendar \
    DeskClock QuickSearchBox Contacts Dialer \
    Email Exchange2 HTMLViewer BookmarkProvider \
    CalendarProvider ContactsProvider TelephonyProvider \
    UserDictionaryProvider WallpaperBackup

# 只保留核心：Launcher3 + Settings + SystemUI + PackageInstaller + LatinIME
SLIMEOF2

# ============================================
# 15. 彻底消除redroid标识（源码级全局替换）
# ============================================
echo "=== 15. redroid标识消除 ==="
# 在所有prop文件里替换
find device/redroid -name "*.prop" -exec sed -i 's/redroid_arm64/umi/g; s/redroid/xiaomi/g' {} \;

# 在设备描述文件里替换（不改文件名，只改内容）
# permissions文件
find device/redroid -name "redroid-removed-permissions.xml" -exec sed -i 's/redroid/device/g' {} \;

# vendor.mk里的产品名
find vendor/redroid -name "*.mk" -exec sed -i 's/PRODUCT_BRAND := redroid/PRODUCT_BRAND := Xiaomi/g' {} \; 2>/dev/null

# 确保没有redroid残留在prop里
echo "残留检查:"
grep -rn "redroid" device/redroid/*.prop 2>/dev/null
grep -rn "redroid_arm64" device/redroid/redroid_arm64/device.mk 2>/dev/null | grep -v "#"

echo "$(date) === 全部补充修改完成 ==="
