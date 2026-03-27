# 当前状态检查报告 — redroid-custom:v5.3

> 检查时间: 2026-03-27
> 容器: redroid-v4, 镜像: redroid-custom:v5.3

---

## 已完成 ✅

| # | 检测项 | 实际值 | 说明 |
|---|--------|--------|------|
| 1 | ro.product.brand | Xiaomi | 编译级写死 |
| 2 | ro.build.fingerprint | Xiaomi/umi/umi:13/... | Momo能读到正确值 |
| 3 | ro.build.tags | release-keys | 编译级 |
| 4 | ro.build.host | build.xiaomi.com | 编译级 |
| 5 | ro.build.date | Sat Sep 15 10:22:18 CST 2023 | 日期伪造成功 |
| 6 | GL_RENDERER | **Adreno650** | ✅ libgl_spoof生效了！ |
| 7 | SELinux getenforce | **Enforcing** | ✅ 之前一直是Disabled |
| 8 | /proc/mounts overlay | 0 | kretprobe过滤 |
| 9 | /.dockerenv | 0字节 | 已清理 |
| 10 | /proc/1/cgroup | 0::/init.scope | 已伪造 |
| 11 | uname -r | 5.10.160-qcom-SDM710+ | 自定义内核 |
| 12 | /proc/cpuinfo | Qualcomm SDM710 | bind mount |
| 13 | /proc/bus/input/devices | 0个rockchip | 已伪装 |
| 14 | 热区 | tsens_tz_sensor0 | 已伪装 |
| 15 | /proc/self/maps mali | 0 | kretprobe替换kgsl |
| 16 | vendor rc文件 | 无redroid开头的rc | 已清洗 |
| 17 | wlan0 | 存在 | dummy接口 |
| 18 | libgl_spoof.so | /system/lib64/ 存在 | 编进了ROM |
| 19 | Zygisk | config=ZYGISK=true | 已配置 |
| 20 | 预装APP | 9个 | 包含抖音等 |

---

## 未完成 ❌ — 需要继续修复

### P0 必须修（直接影响检测结果）

| # | 问题 | 当前值 | 目标值 | 修复方案 |
|---|------|--------|--------|---------|
| 1 | **redroid属性残留34个** | ro.build.description=redroid_arm64-user..., ro.build.product=redroid_arm64, ro.hardware.composer=redroid, 多个分区fingerprint含redroid | 全部Xiaomi/umi | 方案A: provision resetprop清理 / 方案B: 重编ROM overlay修改build.prop |
| 2 | **Magisk/Shamiko缺失** | /sbin/magisk不存在 | Magisk+Shamiko隐藏root | v5.3没有Magisk了？需要重新安装或用带Magisk的镜像 |
| 3 | **LD_PRELOAD未设置** | init.environ.rc无PRELOAD行 | export LD_PRELOAD /system/lib64/libgl_spoof.so | 需要在Docker overlay层修改init.environ.rc或编进ROM |
| 4 | **Widevine DRM缺失** | 只有clearkey | Widevine L3 | 从Pixel镜像提取libwvhidl.so+libwvdrmengine.so |
| 5 | **传感器为空** | No Sensors on device | 7+个虚拟传感器 | 编译virtual sensor HAL或Xposed模块 |
| 6 | **Lineage痕迹** | ro.lineage.device=redroid_arm64, ro.lineage.version=20.0-UNOFFICIAL-redroid, ro.modversion | 删除 | resetprop --delete |

### P1 应该修（降低风控评分）

| # | 问题 | 当前值 | 目标值 | 修复方案 |
|---|------|--------|--------|---------|
| 7 | **默认语言英文** | persist.sys.locale空, ro.product.locale=en-US | zh-CN | resetprop或重编ROM |
| 8 | **eth0仍存在** | eth0@if96可见 | 只有wlan0 | macvlan网络或rename（需不断网方案） |
| 9 | **编译级fingerprint不一致** | ro.build.fingerprint=Xiaomi/umi 但 ro.odm/system_ext/vendor等=redroid | 全分区统一 | 重编ROM修改所有分区build.prop |
| 10 | **ro.boot.redroid_*参数** | redroid_fps/gpu_mode/gpu_node/magisk=4个 | 删除或改名 | resetprop --delete或Docker启动参数改名 |
| 11 | **Canvas指纹** | libgl_spoof有glReadPixels hook但LD_PRELOAD没生效 | 每次不同hash | 修复LD_PRELOAD |
| 12 | **ro.build.description** | redroid_arm64-user 13...eng.flysky...dev-keys | umi-user 13...V14.0.4.0.TJBCNXM release-keys | resetprop或重编 |
| 13 | **ro.product.manufacturer部分分区** | odm/system_ext/vendor_dlkm = redroid | Xiaomi | resetprop或重编 |

### P2 可以后做

| # | 问题 | 说明 |
|---|------|------|
| 14 | 摄像头数量=0 | v4l2loopback模块已编译，Camera HAL适配待做 |
| 15 | Play Integrity | 需要PlayIntegrityFix Magisk模块 |
| 16 | 壁纸 | 系统默认壁纸 |
| 17 | 代理IP池 | 每容器独立出口IP |

---

## 关键发现

### 好消息
1. **GL字符串终于改了！** `GLES: ARM, Adreno650` — libgl_spoof编进了ROM并且SurfaceFlinger层面生效了
2. **SELinux = Enforcing** — 之前一直是Disabled，现在过了
3. **Fingerprint编译级写死** — Momo能读到Xiaomi/umi
4. **编译日期2023年** — 不再是2026年
5. **vendor rc文件已清洗** — 无redroid开头的rc

### 坏消息
1. **redroid属性残留34个** — 大量分区的build.prop没改（odm/system_ext/vendor_dlkm/product）
2. **Magisk消失了** — v5.3镜像可能去掉了Magisk，/sbin/magisk不存在
3. **LD_PRELOAD没在init.environ.rc里** — GL spoof只在SurfaceFlinger进程生效（可能是编译时链接的），APP进程不确定是否生效
4. **GL字符串显示Adreno650但版本号还是Mali的** — `v1.g18p0-01eac0` 是Mali驱动版本，交叉验证仍有矛盾

### 需要验证的
1. 抖音Badger SDK在v5.3上的扫描结果
2. Momo/Ruru在v5.3上的完整检测
3. GL spoof对APP进程是否生效（不只是SurfaceFlinger）

---

## 修复优先级建议

**立即可做（resetprop + provision）：**
- 清理34个redroid属性
- 删除lineage痕迹
- 删除ro.boot.redroid_*
- 设置中文locale

**需要Docker overlay层修改：**
- init.environ.rc加LD_PRELOAD
- 安装Magisk+Shamiko

**需要重编ROM：**
- 所有分区build.prop统一为Xiaomi
- GL版本号修正
- 默认locale=zh-CN
- Widevine L3
- 虚拟传感器HAL
