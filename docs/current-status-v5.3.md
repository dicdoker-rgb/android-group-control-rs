# v5.3 ROM 真实状态 & 待办清单

> 基于 redroid-custom:v5.3 容器 redroid-v4 实际扫描
> 检查时间: 2026-03-27

---

## 一、已在ROM编译时解决的 ✅

| 项目 | 值 | 说明 |
|------|------|------|
| ro.product.brand (全7个分区) | Xiaomi | ✅ 编译级 |
| ro.product.manufacturer (全7个分区) | Xiaomi | ✅ 编译级 |
| ro.build.fingerprint | Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys | ✅ 编译级 |
| ro.bootimage.build.fingerprint | 同上 | ✅ |
| ro.product.build.fingerprint | 同上 | ✅ |
| ro.system.build.fingerprint | 同上 | ✅ |
| ro.vendor.build.fingerprint | 同上 | ✅ |
| ro.build.tags | release-keys | ✅ |
| ro.build.description | umi-user 13 TKQ1.221114.001 V14.0.4.0.TJBCNXM release-keys | ✅ |
| ro.build.product | umi | ✅ |
| ro.build.flavor | umi-user | ✅ |
| ro.build.display.id | V14.0.4.0.TJBCNXM | ✅ |
| ro.build.host | build.xiaomi.com | ✅ |
| ro.build.date | Sat Sep 15 10:22:18 CST 2023 | ✅ 日期伪造 |
| ro.hardware | qcom | ✅ |
| ro.hardware.chipname | sm8250 | ✅ |
| SELinux getenforce | Enforcing | ✅ 之前一直Disabled |
| libgl_spoof.so | /system/lib64/ 存在 | ✅ 编进了ROM |
| provision.sh | /system/bin/ 存在 | ✅ 内置 |
| vendor rc清洗 | 无redroid开头的rc文件 | ✅ |
| persist.sys.locale | zh-CN | ✅ |
| Magisk | /sbin/magisk 存在 | ✅ |
| Zygisk | config=ZYGISK=true | ✅ |
| 预装APP | 10个(抖音/支付宝/华为市场/网易云/Momo/Ruru/Applist/HideMyApplist/Magisk/设备检测) | ✅ |

## 二、宿主机内核模块已解决的 ✅

| 项目 | 值 | 说明 |
|------|------|------|
| /proc/mounts overlay | 0 | kretprobe mounts_filter_v4 |
| /proc/self/maps mali | 0 | kretprobe maps_filter |
| uname -r | 5.10.160-qcom-SDM710+ | 自定义内核 |
| /proc/cpuinfo | Qualcomm SDM710 | bind mount |
| /proc/bus/input/devices | 0个rockchip | bind mount |
| 热区名称 | tsens_tz_sensor0 | bind mount |
| /.dockerenv | 0字节 | provision |
| /proc/1/cgroup | 0::/init.scope | provision |
| wlan0接口 | 存在 | dummy接口 |
| provision自动执行 | ready | ✅ |

---

## 三、未完成 — 按优先级排序

### P0 致命（Momo/Ruru/抖音会直接检测到）

| # | 问题 | 当前值 | 目标值 | 修复方案 |
|---|------|--------|--------|---------|
| 1 | **GL字符串仍是Mali** | `GLES: ARM, Mali-G610, OpenGL ES 3.2 v1.g18p0` | `Qualcomm, Adreno (TM) 650, OpenGL ES 3.2 V@0502.0` | libgl_spoof.so已在/system但**init.environ.rc没有LD_PRELOAD**，需要加`export LD_PRELOAD /system/lib64/libgl_spoof.so` |
| 2 | **3个分区fingerprint仍是redroid** | ro.odm.build.fingerprint, ro.system_ext.build.fingerprint, ro.vendor_dlkm.build.fingerprint = `redroid/redroid_arm64...dev-keys` | Xiaomi/umi...release-keys | 方案A: provision用resetprop覆盖 / 方案B: 重编ROM改这3个分区的build.prop |
| 3 | **ro.hardware.composer=redroid** | redroid | drm 或删除 | resetprop或重编 |
| 4 | **Shamiko未安装** | Magisk有但无Shamiko模块 | Shamiko隐藏root文件 | 下载Shamiko zip → magisk --install-module |
| 5 | **vendor HAL文件名含redroid** | /vendor/lib64/hw/gralloc.redroid.so, hwcomposer.redroid.so | 改名或删除 | 重编ROM或Docker overlay层替换 |
| 6 | **ro.hardware.egl=mali** | mali | adreno (属性层) | resetprop（实际加载仍走mali驱动） |

### P1 高风险（Badger SDK后台打分）

| # | 问题 | 当前值 | 目标值 | 修复方案 |
|---|------|--------|--------|---------|
| 7 | **Widevine DRM缺失** | 只有clearkey | Widevine L3 | 从Pixel镜像提取libwvhidl.so+libwvdrmengine.so |
| 8 | **传感器为空** | No Sensors on device | 7+个虚拟传感器 | 虚拟sensor HAL或Xposed模块 |
| 9 | **Docker启动参数暴露** | androidboot.redroid_gpu_mode=mali, redroid_gpu_node, redroid_fps, redroid_magisk | 删除或改名 | 修改Docker run命令参数名/provision中resetprop --delete |
| 10 | **ro.product.locale=en-US** | en-US | zh-CN | 重编ROM或overlay修改 |
| 11 | **Lineage痕迹** | ro.lineage.device=redroid_arm64, ro.lineage.version, ro.modversion | 删除 | resetprop --delete |
| 12 | **eth0仍存在** | eth0@if96与wlan0共存 | 只有wlan0 | macvlan网络 |
| 13 | **Canvas指纹未生效** | glReadPixels hook在libgl_spoof里但LD_PRELOAD没设 | 每次不同hash | 修复P0#1后自动生效 |
| 14 | **fps=30** | androidboot.redroid_fps=30 | 60 | Docker启动参数改 |

### P2 优化项

| # | 问题 | 说明 |
|---|------|------|
| 15 | 摄像头 | /dev/video10,11存在但Android Camera HAL不识别 |
| 16 | Play Integrity | 需PlayIntegrityFix模块 |
| 17 | 壁纸 | 系统默认 |
| 18 | 代理IP | 每容器独立出口IP |
| 19 | 生活痕迹 | 通讯录/短信/安装时间（provision里有但需确认是否执行） |

---

## 四、关键依赖关系

```
P0#1 LD_PRELOAD修复 → P1#13 Canvas自动生效
P0#4 Shamiko安装 → Momo "环境破坏"消失
P0#1 + P0#2 + P0#3 → Momo fingerprint检查通过
P0#6 egl=adreno → 属性层与GL字符串一致
P1#9 删除redroid启动参数 → 减少属性层暴露面
```

## 五、最快修复路径

**可以通过provision一次性修复的（不需要重编ROM）：**
```bash
# P0#2 fingerprint
resetprop ro.odm.build.fingerprint "Xiaomi/umi/umi:13/..."
resetprop ro.system_ext.build.fingerprint "Xiaomi/umi/umi:13/..."
resetprop ro.vendor_dlkm.build.fingerprint "Xiaomi/umi/umi:13/..."

# P0#3 composer
resetprop ro.hardware.composer drm

# P0#6 egl
resetprop ro.hardware.egl adreno

# P1#9 删redroid启动参数
resetprop --delete ro.boot.redroid_fps
resetprop --delete ro.boot.redroid_gpu_mode
resetprop --delete ro.boot.redroid_gpu_node
resetprop --delete ro.boot.redroid_magisk

# P1#11 删lineage
resetprop --delete ro.lineage.device
resetprop --delete ro.lineage.version
resetprop --delete ro.lineage.display.version
resetprop --delete ro.modversion
```

**必须修改Docker镜像overlay的：**
- P0#1: init.environ.rc加LD_PRELOAD（最关键！）
- P0#5: vendor HAL文件重命名

**必须额外安装的：**
- P0#4: Shamiko模块（从GitHub下载zip）
- P1#7: Widevine（从Pixel镜像提取）

**必须重编ROM的：**
- P1#8: 虚拟传感器HAL
- P1#10: ro.product.locale编译级
