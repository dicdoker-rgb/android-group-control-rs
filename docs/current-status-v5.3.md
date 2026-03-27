# v5.3 ROM 真实状态 & 待办清单

> 基于 redroid-custom:v5.3 容器 redroid-v4 实际扫描
> 检查时间: 2026-03-27
> **此文档是后续开发的唯一参考基准，每次修复后更新**

---

## 一、已在ROM编译时解决的 ✅

| 项目 | 值 | 说明 |
|------|------|------|
| ro.product.brand (全7个分区) | Xiaomi | 编译级 |
| ro.product.manufacturer (全7个分区) | Xiaomi | 编译级 |
| ro.build.fingerprint | Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys | 编译级 |
| ro.bootimage/product/system/vendor.build.fingerprint | 同上 | 编译级(4个分区OK) |
| ro.build.tags | release-keys | 编译级 |
| ro.build.description | umi-user 13 TKQ1.221114.001 V14.0.4.0.TJBCNXM release-keys | 编译级 |
| ro.build.product | umi | 编译级 |
| ro.build.flavor | umi-user | 编译级 |
| ro.build.display.id | V14.0.4.0.TJBCNXM | 编译级 |
| ro.build.host | build.xiaomi.com | 编译级 |
| ro.build.date | Sat Sep 15 10:22:18 CST 2023 | 日期伪造成功 |
| ro.hardware | qcom | 编译级 |
| ro.hardware.chipname | sm8250 | 编译级 |
| SELinux getenforce | Enforcing | 之前一直Disabled,现在OK |
| libgl_spoof.so | /system/lib64/ 存在 | 编进了ROM,但未通过LD_PRELOAD激活 |
| provision.sh | /system/bin/ 存在 | 内置 |
| vendor rc文件 | 无redroid开头的rc文件 | 已清洗 |
| persist.sys.locale | zh-CN | 编译级 |
| Magisk | /sbin/magisk 存在,版本正常 | 已预装 |
| Zygisk | config=ZYGISK=true | 已配置 |
| 预装APP | 10个 | 抖音/支付宝/华为市场/网易云/Momo/Ruru/Applist/HideMyApplist/Magisk/设备检测 |

## 二、宿主机内核模块已解决的 ✅

| 项目 | 值 | 模块 |
|------|------|------|
| /proc/mounts overlay=0 | kretprobe过滤 | mounts_filter_v4.ko |
| /proc/self/maps mali=0 | kretprobe替换kgsl | maps_filter.ko |
| uname -r | 5.10.160-qcom-SDM710+ | 自定义内核 |
| /proc/cpuinfo | Qualcomm SDM710 | provision bind mount |
| /proc/bus/input/devices | 0个rockchip | provision bind mount |
| 热区名称 | tsens_tz_sensor0 | provision bind mount |
| /.dockerenv | 0字节 | provision |
| /proc/1/cgroup | 0::/init.scope | provision |
| wlan0接口 | dummy接口存在 | provision |
| v4l2loopback | /dev/video10,11 | 虚拟摄像头模块 |

---

## 三、未完成 — 统一方案：LSPosed + Xposed 模块

> **关键决策**: LD_PRELOAD方案在Redroid中不可行（init.environ.rc不传给service子进程,改zygote rc会崩boringssl）。
> **统一用 LSPosed + Xposed 模块解决所有运行时hook需求。**
> **前提**: LSPosed必须在v5.3上能正常工作（Zygisk已配true,Magisk已有）。

### 第一步：安装LSPosed（所有Xposed修复的前提）

| 状态 | 说明 |
|------|------|
| ⬜ TODO | 下载LSPosed Zygisk版zip → `magisk --install-module` → 重启zygote → 验证LSPosed Manager可用 |
| 风险 | 之前在老版本Redroid中LSPosed daemon起不来,v5.3需要重新验证 |

### 第二步：通过 Xposed 模块解决的项目

| # | 问题 | 当前值 | 目标值 | Xposed Hook方案 | 状态 |
|---|------|--------|--------|----------------|------|
| 1 | **GL字符串** | ARM, Mali-G610 | Qualcomm, Adreno 650 | Hook `GLES20.glGetString()` + `EGL14.eglQueryString()` 返回Adreno值 | ⬜ TODO |
| 2 | **Canvas指纹** | 固定(Mali渲染特征) | 每次不同hash | Hook `glReadPixels` 注入±2 RGB噪声 | ⬜ TODO |
| 3 | **传感器为空** | No Sensors | 7+个虚拟传感器 | Hook `SensorManager.getSensorList()` 返回虚拟列表 + Hook `dispatchSensorEvent` 注入噪声数据 | ⬜ TODO |
| 4 | **3个分区fingerprint** | odm/system_ext/vendor_dlkm含redroid | Xiaomi/umi | Hook `android.os.Build` 类所有fingerprint字段 + Hook `SystemProperties.get()` 过滤 | ⬜ TODO |
| 5 | **ro.hardware.composer=redroid** | redroid | drm | Hook `SystemProperties.get("ro.hardware.composer")` | ⬜ TODO |
| 6 | **vendor HAL文件名** | gralloc.redroid.so, hwcomposer.redroid.so | 对APP不可见 | Hook `File.exists()` / `File.list()` / `File.getName()` 过滤含redroid的路径 | ⬜ TODO |
| 7 | **Lineage痕迹** | ro.lineage.* / ro.modversion | 空 | Hook `SystemProperties.get()` 返回空 | ⬜ TODO |
| 8 | **ro.hardware.egl=mali** | mali | adreno | Hook `SystemProperties.get("ro.hardware.egl")` | ⬜ TODO |

### 第三步：通过 provision/resetprop 解决的项目（不需要Xposed）

| # | 问题 | 修复命令 | 状态 |
|---|------|---------|------|
| 9 | Docker启动参数(4个redroid_*) | `resetprop --delete ro.boot.redroid_fps/gpu_mode/gpu_node/magisk` | ⬜ TODO |
| 10 | ro.product.locale=en-US | `resetprop ro.product.locale zh-CN` | ⬜ TODO |
| 11 | fps=30 | Docker启动参数改为60 | ⬜ TODO |

### 第四步：需要额外安装的

| # | 项目 | 方案 | 状态 |
|---|------|------|------|
| 12 | **Shamiko** | 下载Shamiko zip → `magisk --install-module` → 隐藏root文件 | ⬜ TODO |
| 13 | **Widevine L3** | 从Pixel镜像提取libwvhidl.so+libwvdrmengine.so → 放入/vendor/lib64/ | ⬜ TODO |
| 14 | **PlayIntegrityFix** | Magisk模块安装 | ⬜ TODO |

### 第五步：P2 优化项（可后做）

| # | 项目 | 说明 |
|---|------|------|
| 15 | 摄像头Camera HAL | v4l2loopback已加载,需Camera HAL适配 |
| 16 | eth0隐藏 | macvlan网络替代bridge |
| 17 | 壁纸 | 预置小米风格 |
| 18 | 代理IP | 每容器独立出口IP |
| 19 | 生活痕迹完善 | 通讯录/短信/安装时间验证 |

---

## 四、Xposed模块编写计划

如果LSPosed能在v5.3工作,需要编写/整合以下Xposed模块:

### 方案A: 一个统一模块搞定所有hook（推荐）

```
com.groupcontrol.antidetect
├── GLSpoof.java          — hook glGetString/eglQueryString → Adreno
├── CanvasPoison.java     — hook glReadPixels → 注入噪声
├── BuildSpoof.java       — hook Build.FINGERPRINT等静态字段
├── PropSpoof.java        — hook SystemProperties.get → 过滤redroid/lineage/mali
├── FileHide.java         — hook File.exists/list → 隐藏*redroid*.so
├── SensorSpoof.java      — hook SensorManager → 返回虚拟传感器列表+噪声
└── xposed_init           — 注册所有hook
```

### 方案B: 用现有开源模块组合

| 需求 | 开源模块 | 说明 |
|------|---------|------|
| GL字符串 | 自研(之前编译的4个Xposed APK) | com.groupcontrol.gpuspoof |
| 属性隐藏 | 应用变量(已有APK) | 通用属性修改工具 |
| 文件隐藏 | HideMyApplist(已安装) | 隐藏应用列表,可能可扩展 |
| 传感器 | com.groupcontrol.sensornoise(之前编译的) | 传感器噪声注入 |
| Root隐藏 | Shamiko | Magisk模块 |

### 之前在3588上编译过的Xposed模块

> 注意: 这些是在之前的会话中用javac→jar→dx→aapt→jarsigner流水线编译的
> 位置: 3588上 `/root/xposed-modules/` (需确认是否还在)

1. `com.groupcontrol.gpuspoof` — Hook GLES20.glGetString (6个Java类)
2. `com.groupcontrol.inputhide` — 隐藏rockchip输入设备 (4个类)
3. `com.groupcontrol.sensornoise` — 传感器噪声注入 (4个类)
4. `com.groupcontrol.signalspoof` — SIM/信号/IMEI欺骗 (10个类)

---

## 五、执行顺序

```
1. 在v5.3上安装LSPosed → 验证是否工作
   ↓ 如果成功
2. 安装之前编译的4个Xposed模块 → 激活
3. 安装Shamiko → 隐藏root
4. provision添加resetprop清理残留属性
5. 运行Momo/Ruru/Applist全面检测
6. 装抖音实测
   ↓ 如果LSPosed不工作
7. 回退到重编ROM方案: 把所有分区build.prop统一+LD_PRELOAD编进init
```

---

## 六、Docker启动参数建议

当前: `androidboot.hardware=qcom androidboot.redroid_gpu_mode=mali androidboot.redroid_gpu_node=/dev/mali0 androidboot.redroid_fps=30 androidboot.redroid_magisk=1`

建议改为: `androidboot.hardware=qcom androidboot.gpu_mode=mali androidboot.gpu_node=/dev/mali0 androidboot.fps=60 androidboot.magisk=1`

去掉所有`redroid_`前缀,fps改60。需要同步修改ROM中读取这些参数的脚本。
