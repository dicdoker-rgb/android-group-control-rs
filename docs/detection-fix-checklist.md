# 真机检测修复清单

> 合并所有检测工具结果（Momo/Ruru/Applist/抖音Badger/手动验证），逐项修复
> 最后更新：2026-03-26

## 检测工具测试结果汇总

| 工具 | 结果 | 说明 |
|------|------|------|
| **Momo 4.4.1** | ❌ 环境异常 | 检测到 Magisk、fingerprint 含 redroid/dev-keys |
| **Ruru 1.1.1** | 8❌ 4✅ | root文件、异常环境、Xposed 全被检测到 |
| **Applist Detector** | 8✅ 1❌ | 仅 Xposed Modules 被检测到 |
| **抖音 Badger SDK** | 功能正常 | 零验证码，但 MediaDrm 失败、Provider 24 失败 |

---

## 一、系统属性伪装（编译时写死）

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 1 | ro.product.brand=Xiaomi | ✅ resetprop | 编译写死 device.mk | ⬜ TODO |
| 2 | ro.product.model=M2001J2C | ✅ resetprop | 编译写死 | ⬜ TODO |
| 3 | ro.product.device=umi | ✅ resetprop | 编译写死 | ⬜ TODO |
| 4 | ro.product.manufacturer=Xiaomi | ✅ resetprop | 编译写死 | ⬜ TODO |
| 5 | ro.build.fingerprint（全分区） | ⚠️ resetprop改了但Momo读到编译值 | **编译写死 BUILD_FINGERPRINT** | ⬜ TODO |
| 6 | ro.build.tags=release-keys | ✅ resetprop | 编译写死 | ⬜ TODO |
| 7 | ro.build.display.id | ✅ resetprop | 编译写死 | ⬜ TODO |
| 8 | ro.build.description | ✅ resetprop | 编译写死 | ⬜ TODO |
| 9 | ro.build.host=build.xiaomi.com | ✅ resetprop | 编译写死 | ⬜ TODO |
| 10 | ro.build.user=android-build | ✅ resetprop | 编译写死 | ⬜ TODO |
| 11 | ro.build.flavor=umi-user | ✅ resetprop | 编译写死 | ⬜ TODO |
| 12 | ro.build.date=2023-09-15 | ⚠️ 实际是2026年 | **BUILD_DATETIME=1694747538** | ⬜ TODO |
| 13 | ro.build.version.incremental | ✅ resetprop | 编译写死 | ⬜ TODO |

## 二、硬件平台

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 14 | ro.hardware=qcom | ✅ resetprop | 编译写死+gpu_config.sh重写 | ⬜ TODO |
| 15 | ro.board.platform=kona | ✅ resetprop | 编译写死 | ⬜ TODO |
| 16 | ro.hardware.chipname=sm8250 | ✅ resetprop | 编译写死 | ⬜ TODO |
| 17 | ro.hardware.egl=adreno(属性) | ✅ resetprop | gpu_config.sh内部用mali对外报adreno | ⬜ TODO |

## 三、GPU/GL 字符串（Momo/Badger核心检测点）

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 18 | glGetString(GL_VENDOR)="Qualcomm" | ❌ 实际返回ARM | **libgl_spoof.so + LD_PRELOAD** | ⬜ TODO |
| 19 | glGetString(GL_RENDERER)="Adreno 650" | ❌ 实际返回Mali-G610 | 同上 | ⬜ TODO |
| 20 | glGetString(GL_VERSION) | ❌ 返回Mali版本 | 同上 | ⬜ TODO |
| 21 | eglQueryString(EGL_VENDOR)="Qualcomm" | ❌ 返回ARM | 同上 | ⬜ TODO |
| 22 | Canvas指纹每次不同 | ❌ 固定(Mali渲染特征) | **glReadPixels hook注入噪声** | ⬜ TODO |

## 四、SELinux / 安全状态

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 23 | getenforce=Enforcing | ❌ 返回Disabled | SELinux permissive+属性enforcing | ⬜ TODO |
| 24 | ro.boot.verifiedbootstate=green | ✅ resetprop | 编译写死 | ⬜ TODO |
| 25 | ro.boot.vbmeta.device_state=locked | ✅ resetprop | 编译写死 | ⬜ TODO |

## 五、Root/Magisk 隐藏（Ruru检测重灾区）

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 26 | /sbin/magisk 等root文件可见 | ❌ Ruru Libc检测到 | **预装Shamiko模块** | ⬜ TODO |
| 27 | Syscall级root文件检测 | ❌ Ruru Syscall检测到 | Shamiko+MagiskHide | ⬜ TODO |
| 28 | Magisk App包名可见 | ✅ 已隐藏(Kitsune) | 保持 | ✅ DONE |
| 29 | Xposed模块可被检测 | ❌ Ruru+Applist检测到 | **Zygisk启用+HideMyApplist** | ⬜ TODO |
| 30 | Abnormal Environment | ❌ Ruru检测到 | Shamiko+Zygisk全套 | ⬜ TODO |
| 31 | PM Command异常 | ❌ Ruru检测到 | HideMyApplist | ⬜ TODO |

## 六、文件系统 redroid 痕迹

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 32 | vendor/etc/init/redroid*.rc | ❌ 文件名含redroid | **重命名: init.device.rc等** | ⬜ TODO |
| 33 | hwcomposer.redroid.so | ❌ HAL模块名 | **重命名: hwcomposer.default.so** | ⬜ TODO |
| 34 | gralloc.redroid.so | ❌ HAL模块名 | **重命名: gralloc.default.so** | ⬜ TODO |
| 35 | 属性中含redroid字样 | ✅ resetprop已清 | 编译时不生成 | ⬜ TODO |

## 七、Docker容器特征

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 36 | /.dockerenv | ✅ bind mount空文件 | provision.sh内置 | ✅ DONE |
| 37 | /proc/1/cgroup含docker | ✅ bind mount | provision.sh内置 | ✅ DONE |
| 38 | /proc/mounts 214个overlay | ✅ kretprobe过滤0 | 内核模块(宿主机) | ✅ DONE |
| 39 | /proc/self/mountinfo | ✅ kretprobe过滤0 | 内核模块(宿主机) | ✅ DONE |
| 40 | /proc/version含rockchip | ✅ 自定义内核 | 宿主机内核已改 | ✅ DONE |
| 41 | uname -r含rockchip | ✅ qcom-SDM710 | 宿主机内核已改 | ✅ DONE |
| 42 | 网络接口eth0 | ⚠️ dummy wlan0创建但eth0仍在 | provision.sh+后续macvlan | ⚠️ 部分 |
| 43 | 热区名称soc-thermal | ✅ bind mount tsens | provision.sh内置 | ✅ DONE |
| 44 | DNS空 | ✅ 223.5.5.5 | provision.sh内置 | ✅ DONE |
| 45 | /proc/cmdline含redroid参数 | ❌ 未处理 | **ROM启动参数清理** | ⬜ TODO |

## 八、/proc 文件伪装

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 46 | /proc/cpuinfo=Qualcomm SDM710 | ✅ bind mount | provision.sh内置 | ✅ DONE |
| 47 | /proc/self/maps无mali | ✅ kretprobe替换kgsl | 内核模块(宿主机) | ✅ DONE |
| 48 | /proc/bus/input/devices | ✅ bind mount qpnp_pon | provision.sh内置 | ✅ DONE |
| 49 | /dev/mali0不暴露 | ✅ maps里已替换 | 但/dev/下仍可见 | ⚠️ 部分 |

## 九、硬件一致性

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 50 | 传感器列表=7+个 | ❌ 空(No Sensors) | **虚拟sensor HAL编译** | ⬜ TODO |
| 51 | 传感器数据有噪声 | ❌ 无数据 | sensor HAL返回基准值+高斯噪声 | ⬜ TODO |
| 52 | 摄像头数量>=2 | ❌ 0个 | v4l2loopback已加载,需Camera HAL | ⚠️ 部分 |
| 53 | MediaDrm/Widevine | ❌ 失败 | **从Pixel提取Widevine L3** | ⬜ TODO |
| 54 | RAM/分辨率匹配机型 | ✅ 8G/1080x2340 | docker参数+画像绑定 | ✅ DONE |
| 55 | 编译日期匹配MIUI | ❌ 2026年 | **BUILD_DATETIME=1694747538** | ⬜ TODO |

## 十、应用生态

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 56 | 预装APP数量正常 | ✅ 6个APP | provision.sh安装 | ✅ DONE |
| 57 | 通讯录有联系人 | ✅ 45-68个 | provision.sh生成 | ✅ DONE |
| 58 | 安装时间分散 | ✅ 随机30-600天 | provision.sh | ✅ DONE |
| 59 | GApps/Play Store | ❌ 未安装 | 预装MindTheGapps或国内商店 | ⬜ TODO |
| 60 | Play Integrity通过 | ❌ 未验证 | PlayIntegrityFix模块 | ⬜ TODO |

## 十一、系统体验

| # | 检测项 | 当前状态 | ROM修复方案 | 状态 |
|---|--------|---------|------------|------|
| 61 | 默认中文 | ⚠️ provision设了但编译时英文 | **编译时zh-CN** | ⬜ TODO |
| 62 | 跳过开机向导 | ❌ 需手动 | **ro.setupwizard.mode=DISABLED** | ⬜ TODO |
| 63 | 默认壁纸 | ❌ 无 | 预置到/system/media/ | ⬜ TODO |
| 64 | ADB默认关闭 | ⚠️ provision设了 | 编译时关 | ⬜ TODO |
| 65 | 状态栏5G/运营商 | ✅ provision设了 | provision.sh | ✅ DONE |

---

## 统计

| 状态 | 数量 | 说明 |
|------|------|------|
| ✅ DONE | 20 | 已通过 |
| ⬜ TODO | 36 | 需ROM编译修复 |
| ⚠️ 部分 | 4 | 部分修复 |
| **总计** | **60** | |

## ROM编译后预期

全部 ⬜ TODO 变 ✅ DONE 后:
- Momo: ✅ 环境正常
- Ruru: 12/12 Not Found
- Applist: 9/9 Not Found
- 抖音Badger: 0失败项, 0验证码
- GL字符串: Qualcomm/Adreno 650
- 传感器: 7个+噪声数据
- fingerprint: 编译级别Xiaomi/umi
