# v5.3 开发计划 — 按执行顺序

> 基于 redroid-custom:v5.3 (redroid-v4容器) 实际扫描结果
> 2026-03-27

## 当前基线

| 项 | 状态 |
|---|------|
| Magisk | ✅ Kitsune R (27001), ZYGISK=true |
| SELinux | ✅ Enforcing |
| GL字符串 | ❌ 仍是 ARM Mali-G610 |
| LSPosed/CPosed | ❌ 未安装 |
| Shamiko | ❌ 未安装 |
| Zygisk实际注入 | ❌ logcat无zygisk日志 |
| redroid残留 | 7个属性 |
| 传感器 | ❌ 空 |
| Widevine | ❌ 只有clearkey |
| /sys RK3588拓扑 | ❌ 11个设备暴露 |
| provision | ✅ 已执行 |
| 预装APP | ✅ 10个 |

---

## 阶段一：让Zygisk真正工作（一切的前提）

Magisk配了ZYGISK=true但logcat没有zygisk日志，说明**Zygisk没有实际注入**。

### 步骤1.1：诊断Zygisk

```bash
# 检查zygisk是否真的注入了
docker exec redroid-v4 logcat -d | grep -iE "zygisk|zygote.*magisk|magisk.*zygote"

# 检查magisk daemon状态
docker exec redroid-v4 /sbin/magisk --denylist ls

# 检查/data/adb/modules有没有zygisk目录
docker exec redroid-v4 ls -la /data/adb/modules/

# 手动触发magisk zygisk重载
docker exec redroid-v4 /sbin/magisk --zygote-restart
```

### 步骤1.2：如果Zygisk不工作

尝试以下修复：
1. 重启zygote: `docker exec redroid-v4 setprop ctl.restart zygote`
2. 检查Magisk版本兼容性（Kitsune R = 27001，可能需要更新）
3. 检查SELinux是否阻止了Zygisk注入
4. 尝试安装 [Zygisk Next](https://github.com/Dr-TSNG/ZygiskNext) 作为替代

### 步骤1.3：验证标准

```
logcat | grep zygisk → 出现 "Zygisk loaded" 或类似日志
/sbin/magisk --denylist ls → 正常返回列表
```

---

## 阶段二：安装CPosed/LSPosed

### 步骤2.1：下载

优先 CPosed（检测绕过增强），备选 JingMatrix LSPosed：
- CPosed: https://github.com/ichason/CPosed
- JingMatrix: https://github.com/JingMatrix/LSPosed/releases

### 步骤2.2：安装

```bash
# 下载到3588宿主机
wget -O /tmp/lsposed.zip <下载链接>
cp /tmp/lsposed.zip /root/redroid-data-v4/local/tmp/

# 容器内安装
docker exec redroid-v4 /sbin/magisk --install-module /data/local/tmp/lsposed.zip

# 重启zygote
docker exec redroid-v4 setprop ctl.restart zygote
sleep 15

# 验证
docker exec redroid-v4 pm list packages | grep lsposed
docker exec redroid-v4 logcat -d | grep -i lsposed | head -5
```

### 步骤2.3：验证标准

```
LSPosed Manager APP 可打开
logcat → "LSPosed daemon started" 或类似
能在LSPosed Manager中看到已安装模块列表
```

---

## 阶段三：安装Magisk模块（不依赖LSPosed的）

### 步骤3.1：Shamiko v1.2.5

```bash
wget -O /tmp/shamiko.zip https://github.com/LSPosed/LSPosed.github.io/releases/download/shamiko-v1.2.5/Shamiko-v1.2.5-release.zip
cp /tmp/shamiko.zip /root/redroid-data-v4/local/tmp/
docker exec redroid-v4 /sbin/magisk --install-module /data/local/tmp/shamiko.zip
```

### 步骤3.2：iUnlockerGL v1.1.5（GL字符串）

```bash
# 从 https://github.com/i-Taylo/iUnlockerGL/releases 下载
wget -O /tmp/iunlockergl.zip <链接>
docker exec redroid-v4 /sbin/magisk --install-module /data/local/tmp/iunlockergl.zip
```

### 步骤3.3：Zygisk-Assistant

```bash
wget -O /tmp/zygisk-assistant.zip https://github.com/snake-4/Zygisk-Assistant/releases/latest/download/ZygiskAssistant.zip
docker exec redroid-v4 /sbin/magisk --install-module /data/local/tmp/zygisk-assistant.zip
```

### 步骤3.4：重启并验证

```bash
docker exec redroid-v4 setprop ctl.restart zygote
sleep 15

# GL字符串
docker exec redroid-v4 dumpsys SurfaceFlinger | grep "GLES:"
# 期望: Qualcomm, Adreno 650

# Momo测试
docker exec redroid-v4 am start -n io.github.vvb2060.mahoshojo/.home.HomeActivity
# 截图检查
```

---

## 阶段四：安装Xposed模块（依赖LSPosed）

### 步骤4.1：HideMyAndroid（一站式方案）

```bash
# 从 https://modules.lsposed.org/module/com.wowsoftware.hidemyandroid/ 下载APK
# 安装
docker exec redroid-v4 pm install /data/local/tmp/hidemyandroid.apk

# 在LSPosed Manager中：
# 1. 启用HideMyAndroid模块
# 2. 勾选目标APP（抖音、Momo等）
# 3. 配置画像信息
```

配置项：
- 设备型号：Xiaomi M2001J2C
- IMEI/Serial/MAC：从画像读取
- GPU：Qualcomm Adreno 650
- 传感器：启用全部7个虚拟传感器
- 隐藏root/模拟器/VPN

### 步骤4.2：Device Emulator v5.0.2（备选）

如果HideMyAndroid不够，补充安装：
- https://modules.lsposed.org/module/dev.device.emulator/

### 步骤4.3：自研模块（补充）

如果以上模块有覆盖不到的，用之前编译的4个模块：
```bash
# 检查模块是否还在3588上
ls /root/xposed-modules/

# 安装
docker exec redroid-v4 pm install /data/local/tmp/gpuspoof.apk
docker exec redroid-v4 pm install /data/local/tmp/signalspoof.apk
docker exec redroid-v4 pm install /data/local/tmp/sensornoise.apk
docker exec redroid-v4 pm install /data/local/tmp/inputhide.apk
```

---

## 阶段五：provision脚本更新

### 步骤5.1：resetprop清理残留

在provision.sh中添加：
```bash
# 清理redroid残留
resetprop ro.hardware.composer drm
resetprop --delete ro.boot.redroid_gpu_mode
resetprop --delete ro.boot.redroid_gpu_node
resetprop --delete ro.boot.redroid_fps
resetprop --delete ro.boot.redroid_magisk

# 清理3个分区fingerprint
FP="Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys"
resetprop ro.odm.build.fingerprint "$FP"
resetprop ro.system_ext.build.fingerprint "$FP"
resetprop ro.vendor_dlkm.build.fingerprint "$FP"

# 清理lineage
resetprop --delete ro.lineage.build.version.plat.rev
resetprop --delete ro.lineage.build.version.plat.sdk
resetprop --delete ro.lineagelegal.url
```

### 步骤5.2：/sys/devices/platform/ RK3588拓扑隐藏

```bash
# bind mount覆盖关键设备名
for dev in fb000000.gpu rknpu_dev.14.auto rkvenc-ccu; do
    if [ -d /sys/devices/platform/$dev ]; then
        mkdir -p /data/fake_sys/$dev
        mount -o bind /data/fake_sys/$dev /sys/devices/platform/$dev/uevent 2>/dev/null
    fi
done
```

或者写新的kretprobe内核模块过滤/sys目录枚举。

### 步骤5.3：安装来源伪造

```bash
# 让已安装APP显示来自Google Play
for pkg in $(pm list packages -3 | cut -d: -f2); do
    pm set-installer $pkg com.android.vending 2>/dev/null
done
```

---

## 阶段六：Widevine DRM安装

### 步骤6.1：从Pixel镜像提取

在Mac上（能访问Google CDN）：
```bash
wget https://dl.google.com/dl/android/aosp/panther-tq3a.230901.001-factory-36159691.zip
# 解压 → 提取vendor.img → mount → 复制libwvhidl.so + libwvdrmengine.so
```

### 步骤6.2：部署到容器

```bash
scp libwvhidl.so libwvdrmengine.so root@192.168.31.9:/tmp/
# 放入容器vendor目录（通过宿主机data卷+bind mount）
```

---

## 阶段七：全面检测验证

### 步骤7.1：运行检测工具

```bash
# Momo
docker exec redroid-v4 am start -n io.github.vvb2060.mahoshojo/.home.HomeActivity
# → 截图检查是否"环境正常"

# Ruru
docker exec redroid-v4 am start -n com.byxiaorun.detector/.MainActivity
# → 截图检查12/12 Not Found

# Applist Detector
docker exec redroid-v4 am start -n icu.nullptr.applistdetector/.MainActivity
# → 截图检查9/9 Not Found

# 抖音
docker exec redroid-v4 am start -n com.ss.android.ugc.aweme/.splash.SplashActivity
# → 无验证码，Badger日志0失败
```

### 步骤7.2：抓Badger SDK日志

```bash
logcat -c
# 启动抖音后等30秒
logcat -d | grep -iE "badger|bbox|MediaDrm|HookDetect|nativeGetReport"
# → nativeGetReport isFull=1, 0 failures
```

---

## 执行时间估计

| 阶段 | 预计耗时 | 阻塞条件 |
|------|---------|---------|
| 一、Zygisk诊断 | 15分钟 | 如果不工作需要排查 |
| 二、LSPosed安装 | 15分钟 | 依赖阶段一 |
| 三、Magisk模块 | 20分钟 | 下载+安装+验证 |
| 四、Xposed模块 | 30分钟 | 依赖阶段二 |
| 五、provision更新 | 15分钟 | 无阻塞 |
| 六、Widevine | 30分钟 | Pixel镜像下载 |
| 七、检测验证 | 20分钟 | 依赖全部 |
| **总计** | **~2.5小时** | |

---

## 关键风险

| 风险 | 影响 | 应对 |
|------|------|------|
| Zygisk在v5.3仍然不工作 | 所有Xposed模块无法使用 | 尝试Zygisk Next替代/排查SELinux/Magisk版本 |
| LSPosed在容器中daemon不启动 | 无法hook APP层API | 尝试CPosed/LSPosed-Irena/降级版本 |
| iUnlockerGL不兼容Mali驱动 | GL字符串改不了 | 回退到Xposed hook方案 |
| HideMyAndroid功能不全 | 需要多个模块组合 | Device Emulator + 自研模块补充 |

---

## 阶段一结果：Magisk Module Mount 不可用

### 根因
Magisk Kitsune在v5.3 Redroid容器中的module mount系统不工作：
- `/sbin/.magisk` 是symlink到 `/root/.magisk`
- worker tmpfs可以手动创建但Magisk daemon在init阶段创建失败
- `--post-fs-data` 触发后依然无法mount modules到system
- ZygiskNext、LSPosed、Shamiko都安装到了`/data/adb/modules/`但没有被加载

### 已找到的替代方案
1. **GL spoof**: ✅ 通过app_process64 wrapper解决（不需要Zygisk）
2. **Root隐藏**: ⚠️ bind mount可隐藏/sbin和/data/adb和magiskd进程名，但Momo仍检测到系统修改行为
3. **属性伪装**: ✅ resetprop可解决（不需要Xposed）

### 需要的方案
Momo的"检测到破坏行为(MagiskHide)"是检测Magisk对系统的修改痕迹（sepolicy注入等），只有Shamiko（需要Zygisk注入）能绕过。

可能的解决路径：
1. **换KernelSU** — 内核级root，不修改系统分区，天然更难检测
2. **修复Magisk集成** — 在ROM编译时正确设置Magisk的init脚本让module mount工作
3. **自定义Zygisk注入** — 不走Magisk module系统，直接从宿主机注入zygisk so到zygote
4. **完全不用Magisk** — 群控场景其实不需要root（resetprop通过Docker启动参数也能实现），去掉Magisk反而更干净
