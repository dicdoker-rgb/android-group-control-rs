# ROM 重编译计划

> 目标：一次编译解决所有"需ROM层面修复"的检测项，产出可直接部署的 Docker 镜像

## 当前问题

运行时 resetprop + 内核模块已经让抖音零验证码运行，但以下问题**只能通过编ROM解决**：

| 问题 | 原因 | 影响 |
|------|------|------|
| GL_VENDOR=ARM, GL_RENDERER=Mali-G610 | glGetString 从 GPU 寄存器读，不走属性 | 交叉验证矛盾，后台扣分 |
| Widevine DRM 缺失 | clearkey HAL 不支持 Widevine UUID | Badger feature 22 失败 |
| /vendor/etc/init/redroid*.rc | 文件名含 redroid | APP 扫描文件系统暴露 |
| hwcomposer.redroid.so / gralloc.redroid.so | HAL 模块名含 redroid | 同上 |
| Zygisk/LSPosed 不工作 | Magisk module overlay 在 Redroid 里失效 | 无法用 Xposed 模块 |
| Canvas 指纹固定 | 需要 Zygisk 模块注入 glReadPixels hook | WebGL 指纹追踪 |
| /system 只读 | Docker overlay 不允许运行时写 /system | LD_PRELOAD 无法部署 |
| SELinux 实际 Disabled | 内核启动参数未启用 | getenforce 暴露 |
| 传感器为空 | 无 sensor HAL | SensorManager 返回空列表 |

## 编译方案

### 基础：cnflysky/redroid-rk3588 LineageOS 20 源码树 + overlay 定制

不从零编译 AOSP，在已验证能跑的 cnflysky 源码树上改。

### 编译环境

```
阿里云 ecs.c7.8xlarge (32核 64G)
Ubuntu 22.04
磁盘: 500G SSD
预计耗时: 3-4 小时（含源码同步）
```

---

## 改动清单（共 12 项）

### 1. GL 字符串欺骗（最关键）

**文件**: `vendor/redroid_ext/gpu_config.sh` + 新增 `libgl_spoof.so`

**方案**: 在 ROM 中预置 GL hook 库，通过 init.environ.rc 设置全局 LD_PRELOAD

```bash
# vendor/redroid_ext/gpu_config.sh 改为:
#!/system/bin/sh
if [ -e /dev/mali0 ]; then
    setprop ro.hardware.egl mali        # 内部用 mali 加载驱动
    setprop ro.hardware.gralloc gbm
fi
# 对外属性报 Adreno（不影响驱动加载）
setprop persist.sys.gpu.vendor Qualcomm
setprop persist.sys.gpu.renderer "Adreno (TM) 650"
```

```rc
# init.environ.rc 添加:
export LD_PRELOAD /system/lib64/libgl_spoof.so
```

**libgl_spoof.so 源码**: `kernel-modules/gl_spoof.c`（已写好，NDK 编译）

hook 函数:
- `glGetString(GL_VENDOR)` → "Qualcomm"
- `glGetString(GL_RENDERER)` → "Adreno (TM) 650"
- `glGetString(GL_VERSION)` → "OpenGL ES 3.2 V@0502.0"
- `eglQueryString(EGL_VENDOR)` → "Qualcomm Inc."

编译进 `/system/lib64/libgl_spoof.so`（64位）+ `/system/lib/libgl_spoof.so`（32位）

### 2. Widevine L3 DRM

**来源**: 从 Google Pixel factory image 提取（x86 服务器可以访问 dl.google.com）

```bash
# 下载 Pixel 7 factory image
wget https://dl.google.com/dl/android/aosp/panther-tq3a.230901.001-factory.zip
# 解压 vendor.img，提取:
# /vendor/lib64/libwvhidl.so
# /vendor/lib64/mediadrm/libwvdrmengine.so
# /vendor/etc/init/android.hardware.drm@1.4-service.widevine.rc
# /vendor/bin/hw/android.hardware.drm@1.4-service.widevine
```

**部署位置**: 直接放入 vendor overlay

### 3. 文件名全局清洗（去 redroid）

```bash
# 批量重命名
mv vendor/etc/init/redroid.common.rc     → vendor/etc/init/init.device.rc
mv vendor/etc/init/redroid.ext.rc        → vendor/etc/init/init.hardware.rc
mv vendor/etc/init/redroid.c2.rc         → vendor/etc/init/init.media.rc
mv vendor/etc/init/redroid.omx.rc        → vendor/etc/init/init.omx.rc
rm vendor/etc/init/redroid.legacy.rc

# HAL 模块重命名
mv hwcomposer.redroid.so  → hwcomposer.default.so
mv gralloc.redroid.so     → gralloc.default.so

# 源码全局替换
find device/redroid vendor/redroid* -type f \
  \( -name "*.mk" -o -name "*.rc" -o -name "*.prop" -o -name "*.bp" \) \
  -exec sed -i 's/redroid_arm64/umi/g; s/redroid/xiaomi/g' {} \;

# 但保留 gpu_config.sh 内的 Mali 设备检测逻辑
```

### 4. build.prop 全量写死

**所有分区** (system/vendor/product/odm/system_ext/vendor_dlkm):

```properties
ro.product.brand=Xiaomi
ro.product.model=M2001J2C
ro.product.device=umi
ro.product.manufacturer=Xiaomi
ro.product.name=umi
ro.build.fingerprint=Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys
ro.build.tags=release-keys
ro.build.type=user
ro.build.display.id=V14.0.4.0.TJBCNXM
ro.build.description=umi-user 13 TKQ1.221114.001 V14.0.4.0.TJBCNXM release-keys
ro.build.host=build.xiaomi.com
ro.build.user=android-build
ro.build.flavor=umi-user
ro.build.version.incremental=V14.0.4.0.TJBCNXM
ro.build.date=Sat Sep 15 10:22:18 CST 2023
ro.build.date.utc=1694747538
ro.board.platform=kona
ro.hardware=qcom
ro.hardware.chipname=sm8250
ro.hardware.egl=adreno
ro.hardware.vulkan=adreno
ro.hardware.composer=drm
ro.boot.hardware=qcom
ro.boot.selinux=enforcing
ro.boot.verifiedbootstate=green
ro.boot.vbmeta.device_state=locked
ro.debuggable=0
ro.secure=1
```

**注意**: `ro.hardware` 内部仍用于 GPU 驱动加载，gpu_config.sh 重写后不依赖这个值。

### 5. GPU 初始化脚本重写

**核心改动**: gpu_config.sh 不再依赖 `ro.hardware` 来判断加载 Mali

```bash
#!/system/bin/sh
# 直接检测硬件节点，不依赖 ro.hardware
if [ -e /dev/mali0 ]; then
    # 内部使用 Mali 驱动
    setprop ro.hardware.egl mali
    setprop ro.hardware.gralloc gbm
    # 创建 symlinks 兼容 qcom HAL 路径
    ln -sf /vendor/lib64/hw/hwcomposer.default.so /vendor/lib64/hw/hwcomposer.qcom.so 2>/dev/null
    ln -sf /vendor/lib64/hw/gralloc.default.so /vendor/lib64/hw/gralloc.qcom.so 2>/dev/null
fi
```

### 6. SELinux 配置

```makefile
# device.mk
BOARD_KERNEL_CMDLINE += androidboot.selinux=permissive
# 属性层报 enforcing，实际 permissive（Magisk 标准做法）
```

启动后 Magisk 的 resetprop 设 `ro.boot.selinux=enforcing`。

### 7. 预装 provision.sh + first_boot.rc

```rc
# system/etc/init/first_boot.rc
service first_boot /system/bin/sh /system/bin/provision.sh
    class late_start
    user root
    oneshot
    disabled

on property:sys.boot_completed=1
    start first_boot
```

`/system/bin/provision.sh` = 我们的 `scripts/provision_full.sh`

### 8. Canvas 指纹毒化

编译 Zygisk C++ 模块，hook `glReadPixels`:

```cpp
// 在每次 glReadPixels 返回后注入 ±2 RGB 噪声
// 每 97 个像素采样一次，种子基于帧尺寸
// 使得每次 Canvas 读取产生不同哈希
```

因为 ROM 里 LD_PRELOAD 已经全局生效，直接把 Canvas hook 合并进 `libgl_spoof.so`。

### 9. 虚拟传感器

**方案 A**: 编译 virtual sensor HAL

```makefile
# device.mk
PRODUCT_PACKAGES += android.hardware.sensors@2.1-service.virtual
```

返回 7 个基础传感器:
- 加速度计 (TYPE_ACCELEROMETER)
- 陀螺仪 (TYPE_GYROSCOPE)
- 磁力计 (TYPE_MAGNETIC_FIELD)
- 光线 (TYPE_LIGHT)
- 接近 (TYPE_PROXIMITY)
- 气压 (TYPE_PRESSURE)
- 温度 (TYPE_AMBIENT_TEMPERATURE)

数据：基础值 + 高斯白噪声

**方案 B**: 如果 HAL 编译复杂，用 Xposed 模块 hook SensorManager（Zygisk 在自编 ROM 里应该能工作）

### 10. 预装 Magisk + Zygisk + 模块

```
/system/etc/init/magisk.rc          → Magisk 自启
/data/adb/magisk/config             → ZYGISK=true
/data/adb/modules/shamiko/          → Root 隐藏
/data/adb/modules/hide_my_applist/  → 应用列表隐藏
/data/adb/modules/gl_spoof/         → GL 字符串（备用）
```

### 11. 预装常用 APP 到共享卷

不编进 ROM（太大），通过共享 APK 卷 + provision.sh 安装:

```bash
# Docker 启动参数:
-v /root/shared-apks:/data/shared-apks:ro

# provision.sh 中:
for apk in /data/shared-apks/*.apk; do
    pm install -r "$apk"
done
```

### 12. 编译日期伪造

```bash
# 编译前修改时间戳
export BUILD_DATETIME=1694747538  # 2023-09-15
export BUILD_NUMBER=V14.0.4.0.TJBCNXM
```

---

## 编译流程

```bash
# ===== 阶段 0: 环境准备 (15分钟) =====
sudo apt install -y openjdk-17-jdk-headless repo git-lfs \
  bc bison build-essential ccache curl flex g++-multilib \
  gcc-multilib gnupg gperf libelf-dev libncurses5 \
  libssl-dev libxml2-utils lzop zip zlib1g-dev

# 安装 NDK（编译 libgl_spoof.so）
wget https://dl.google.com/android/repository/android-ndk-r26c-linux.zip
unzip android-ndk-r26c-linux.zip

# ===== 阶段 1: 源码获取 (1小时) =====
mkdir ~/redroid-build && cd ~/redroid-build
repo init -u https://android.googlesource.com/platform/manifest \
  --git-lfs --depth=1 -b android-13.0.0_r74
git clone https://github.com/nicknsy/redroid-rockchip-platform_manifests.git \
  .repo/local_manifests -b redroid-13.0.0_r74
repo sync -c -j$(nproc) --no-tags

# ===== 阶段 2: 应用定制修改 (30分钟) =====

# 2a. build.prop 全量伪装
# 修改 device/redroid/device.mk
cat >> device/redroid/device.mk << 'EOF'
PRODUCT_BRAND := Xiaomi
PRODUCT_MODEL := M2001J2C
PRODUCT_DEVICE := umi
PRODUCT_MANUFACTURER := Xiaomi
PRODUCT_NAME := umi
EOF

# 2b. 文件名清洗
./apply_renames.sh  # 我们准备的脚本

# 2c. gpu_config.sh 重写
cp our_gpu_config.sh vendor/redroid_ext/gpu_config.sh

# 2d. 编译 libgl_spoof.so (NDK)
$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang \
  -shared -fPIC -O2 -o system/lib64/libgl_spoof.so gl_spoof.c -ldl
$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi33-clang \
  -shared -fPIC -O2 -o system/lib/libgl_spoof.so gl_spoof.c -ldl

# 2e. init.environ.rc 添加 LD_PRELOAD
echo '    export LD_PRELOAD /system/lib64/libgl_spoof.so' >> \
  system/core/rootdir/init.environ.rc.in

# 2f. 提取 Widevine (从 Pixel factory image)
./extract_widevine.sh

# 2g. provision.sh 内置
cp provision_full.sh system/bin/provision.sh
cp first_boot.rc system/etc/init/

# 2h. 编译日期伪造
export BUILD_DATETIME=1694747538

# ===== 阶段 3: 编译 (30-60分钟) =====
. build/envsetup.sh
lunch redroid_arm64-userdebug
m -j$(nproc)

# ===== 阶段 4: 打包 Docker 镜像 (10分钟) =====
cd out/target/product/redroid_arm64
sudo mount system.img system -o ro
sudo mount vendor.img vendor -o ro
sudo tar --xattrs -c vendor -C system --exclude="./vendor" . | \
  docker import --platform=linux/arm64 \
  -c 'ENTRYPOINT ["/init", "androidboot.hardware=qcom"]' \
  - redroid-custom:v4.0-anti-detect

# 推送到 3588
docker save redroid-custom:v4.0-anti-detect | \
  ssh root@192.168.31.9 'docker load'

# ===== 阶段 5: 验证 (30分钟) =====
# 在 3588 上用新镜像创建容器，跑 43 项检测 + 抖音实测
```

## 预期结果

编译完成后:

| 检测项 | 当前 | 编译后 |
|--------|------|--------|
| GL_VENDOR/RENDERER | ARM/Mali-G610 | **Qualcomm/Adreno 650** |
| Widevine DRM | 失败 | **L3 通过** |
| vendor 文件名 | redroid.*.rc | **init.device.rc** |
| Canvas 指纹 | 固定 | **每次不同** |
| SELinux | Disabled | **Permissive(报Enforcing)** |
| Zygisk | 不工作 | **正常工作** |
| 传感器 | 空 | **7个虚拟传感器** |
| /system 可写 | 否 | **LD_PRELOAD 编译时写入** |
| provision | 需 docker exec | **开机自动执行** |
| 编译日期 | 2026年 | **2023年9月** |

抖音 Badger SDK 扫描预期: 0 失败项, 0 验证码

## 需要准备的文件

在开服务器前准备好，上传到编译机:

1. `gl_spoof.c` — GL hook 源码 ✅ 已有
2. `provision_full.sh` — provision 脚本 ✅ 已有
3. `first_boot.rc` — 开机自启 rc ✅ 需创建
4. `apply_renames.sh` — 文件名清洗脚本 ✅ 需创建
5. `extract_widevine.sh` — Widevine 提取脚本 ✅ 需创建
6. `device_profile.json` — 默认画像模板 ✅ 已有
7. `canvas_poison.c` — Canvas 毒化合并到 gl_spoof ✅ 需合并

## 时间预估

| 步骤 | 耗时 |
|------|------|
| 开服务器+环境配置 | 15 分钟 |
| repo sync | 1 小时 |
| 应用修改 | 30 分钟 |
| 编译 | 30-60 分钟 |
| 打包+传输到 3588 | 15 分钟 |
| 验证测试 | 30 分钟 |
| **总计** | **3-4 小时** |
