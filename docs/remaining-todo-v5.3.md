# v5.3 剩余待办 — 完全模拟真机

> 目标：所有检测工具（Momo/Ruru/Applist/抖音Badger）全部通过，与真机无差异
> 基于 redroid-custom:v5.3 实际扫描 + 社区最新方案
> 更新时间：2026-03-27

---

## 技术路线：LSPosed/CPosed + Xposed模块 + Magisk模块

不再重编ROM，全部通过模块化方案解决。

---

## 一、框架层安装（所有模块的前提）

| # | 组件 | 版本 | 来源 | 说明 | 状态 |
|---|------|------|------|------|------|
| 1 | **CPosed** 或 **JingMatrix/LSPosed** | CPosed最新 / LSPosed v1.11.0 | [CPosed](https://github.com/ichason/CPosed) / [JingMatrix](https://github.com/JingMatrix/LSPosed/releases) | LSPosed增强fork，专注检测绕过。CPosed优先，不行换JingMatrix | ⬜ |
| 2 | **Shamiko** | v1.2.5 | [GitHub](https://github.com/LSPosed/LSPosed.github.io/releases) | Magisk模块，隐藏root/Zygisk/Magisk痕迹 | ⬜ |
| 3 | **Zygisk-Assistant** | 最新 | [GitHub](https://github.com/snake-4/Zygisk-Assistant/releases) | 开源root隐藏，Shamiko的补充 | ⬜ |

---

## 二、GPU/GL字符串欺骗

| # | 问题 | 当前值 | 目标值 | 方案 | 状态 |
|---|------|--------|--------|------|------|
| 4 | GL_VENDOR | ARM | Qualcomm | **方案A**: [iUnlockerGL](https://github.com/i-Taylo/iUnlockerGL) v1.1.5 (Magisk模块，不需要LSPosed) | ⬜ |
| 5 | GL_RENDERER | Mali-G610 | Adreno (TM) 650 | 同上 | ⬜ |
| 6 | GL_VERSION | v1.g18p0 (Mali版本号) | V@0502.0 (Adreno版本号) | 同上 | ⬜ |
| 7 | EGL_VENDOR | ARM | Qualcomm Inc. | 同上或Xposed hook EGL14.eglQueryString() | ⬜ |
| 8 | Canvas指纹 | 固定(Mali渲染特征) | 每次不同hash | libgl_spoof.so已有glReadPixels hook，需通过iUnlockerGL或LSPosed激活 | ⬜ |

> **备选方案B**: 如果iUnlockerGL不work，用LSPosed + 我们之前编译的 `com.groupcontrol.gpuspoof` 模块
> **备选方案C**: [HideMyAndroid](https://modules.lsposed.org/module/com.wowsoftware.hidemyandroid/) 据称包含GPU欺骗

---

## 三、SIM卡/信号/通信 完全模拟

| # | 检测项 | 当前值 | 目标值 | Hook点 | 状态 |
|---|--------|--------|--------|--------|------|
| 9 | SIM状态 | 属性设了READY但API可能返回ABSENT | READY | `TelephonyManager.getSimState()` | ⬜ |
| 10 | 运营商名称 | 属性设了但API层未hook | 中国移动/中国联通/中国电信(随画像) | `TelephonyManager.getNetworkOperatorName()` / `getSimOperatorName()` | ⬜ |
| 11 | 网络类型 | 属性NR但API可能返回UNKNOWN | NR/LTE | `TelephonyManager.getNetworkType()` / `getDataNetworkType()` | ⬜ |
| 12 | IMEI | 属性设了但API层 | 画像中的IMEI | `TelephonyManager.getDeviceId()` / `getImei()` | ⬜ |
| 13 | 手机号 | 属性设了 | 画像中的号码 | `TelephonyManager.getLine1Number()` | ⬜ |
| 14 | ICCID | 空 | 伪造SIM卡序列号 | `TelephonyManager.getSimSerialNumber()` | ⬜ |
| 15 | 基站信息 | 空 | 伪造LTE/NR基站 | `TelephonyManager.getAllCellInfo()` → 返回`CellInfoLte`/`CellInfoNr`对象 | ⬜ |
| 16 | 信号强度 | 空/无信号 | -70~-90 dBm (正常信号) | `TelephonyManager.getSignalStrength()` → 返回`SignalStrength`对象 | ⬜ |
| 17 | 双卡信息 | 属性设了dsds | 双卡双待 | `SubscriptionManager.getActiveSubscriptionInfoList()` → 返回2个`SubscriptionInfo` | ⬜ |
| 18 | MCC/MNC | 属性46000 | 460/00(移动) 460/01(联通) 460/11(电信) | `TelephonyManager.getNetworkOperator()` / `getSimOperator()` | ⬜ |

> **方案**:
> - 优先用 [HideMyAndroid](https://modules.lsposed.org/module/com.wowsoftware.hidemyandroid/) 或 [Device Emulator](https://modules.lsposed.org/module/dev.device.emulator/) v5.0.2
> - 不够用再补我们自研的 `com.groupcontrol.signalspoof`（已编译10个类，在3588 /root/xposed-modules/）

---

## 四、传感器 完全模拟

| # | 传感器 | 当前 | 目标值 | 说明 | 状态 |
|---|--------|------|--------|------|------|
| 19 | 加速度计 TYPE_ACCELEROMETER | 不存在 | x=0.1, y=0.2, z=9.81 ± 高斯噪声 | 重力方向+微抖动 | ⬜ |
| 20 | 陀螺仪 TYPE_GYROSCOPE | 不存在 | x,y,z = 0 ± 0.001 rad/s | 静止状态微噪声 | ⬜ |
| 21 | 磁力计 TYPE_MAGNETIC_FIELD | 不存在 | x=25, y=-10, z=-45 μT ± 噪声 | 地磁场模拟 | ⬜ |
| 22 | 光线传感器 TYPE_LIGHT | 不存在 | 200-500 lux | 室内正常光线 | ⬜ |
| 23 | 接近传感器 TYPE_PROXIMITY | 不存在 | 5.0 cm (远离) | 未贴脸 | ⬜ |
| 24 | 气压计 TYPE_PRESSURE | 不存在 | 1013.25 hPa ± 0.5 | 标准大气压 | ⬜ |
| 25 | 温度 TYPE_AMBIENT_TEMPERATURE | 不存在 | 25°C ± 0.3 | 室温 | ⬜ |

> **方案A**: [HideMyAndroid](https://modules.lsposed.org/module/com.wowsoftware.hidemyandroid/) 据称包含"realistic sensor data"
> **方案B**: 我们自研的 `com.groupcontrol.sensornoise`（已编译4个类）
> **方案C**: [XposedFakeLocation](https://github.com/noobexon1/XposedFakeLocation) 包含传感器数据伪造
>
> Hook点:
> - `SensorManager.getSensorList()` → 返回虚拟传感器列表
> - `SensorManager.getDefaultSensor()` → 返回对应虚拟传感器
> - `SensorEventListener.onSensorChanged()` → 注入伪造数据+高斯噪声

---

## 五、设备标识/属性 完全伪装

| # | 问题 | 当前值 | 目标值 | 方案 | 状态 |
|---|------|--------|--------|------|------|
| 26 | 3个分区fingerprint含redroid | odm/system_ext/vendor_dlkm | Xiaomi/umi | Hook `SystemProperties.get()` 或 `Build` 类字段 | ⬜ |
| 27 | ro.hardware.composer=redroid | redroid | drm | Hook `SystemProperties.get()` | ⬜ |
| 28 | ro.hardware.egl=mali | mali | adreno | Hook `SystemProperties.get()` | ⬜ |
| 29 | gralloc.redroid.so / hwcomposer.redroid.so | 文件存在 | 对APP不可见 | Hook `File.exists()` / `File.list()` / `File.getName()` 过滤含redroid路径 | ⬜ |
| 30 | Lineage痕迹(4个属性) | ro.lineage.* | 空 | Hook `SystemProperties.get()` 返回空 | ⬜ |
| 31 | Docker启动参数(4个redroid_*) | ro.boot.redroid_* | 不可见 | Hook `SystemProperties.get()` 返回空 | ⬜ |
| 32 | Android ID | 需每容器唯一 | 画像中的值 | `Settings.Secure.getString(ANDROID_ID)` hook 或 provision 已设 | ⬜ |
| 33 | MAC地址 | 需每容器唯一 | 画像中的值 | `WifiInfo.getMacAddress()` hook | ⬜ |
| 34 | 序列号 | 需每容器唯一 | 画像中的值 | `Build.SERIAL` / `Build.getSerial()` hook | ⬜ |

> **方案**: [Device Emulator](https://modules.lsposed.org/module/dev.device.emulator/) v5.0.2 或 [SpoofMyDevice](https://xdaforums.com/t/app-xposed-spoofmydevice-spoof-your-android-device.4783119/) v2.0.2 或 [Android Faker](https://github.com/Android1500/AndroidFaker)

---

## 六、Root/环境隐藏

| # | 问题 | 方案 | 状态 |
|---|------|------|------|
| 35 | Magisk root文件可被检测 | **Shamiko** v1.2.5 | ⬜ |
| 36 | Zygisk痕迹 | Shamiko + **Zygisk-Assistant** | ⬜ |
| 37 | Xposed/LSPosed模块可被检测 | **HideMyApplist**(已安装) + [FakeXposed](https://github.com/sanfengAndroid/FakeXposed) | ⬜ |
| 38 | Magisk Manager包名可被检测 | Magisk Kitsune自带包名随机化 | ✅ 已有 |
| 39 | su binary | Shamiko隐藏 | ⬜ |
| 40 | /proc/self/maps Zygisk so | Shamiko隐藏 | ⬜ |

---

## 七、Widevine DRM

| # | 问题 | 方案 | 状态 |
|---|------|------|------|
| 41 | MediaDrm(WIDEVINE_UUID)失败 | 从Pixel factory image提取 libwvhidl.so + libwvdrmengine.so 放入 /vendor/lib64/ | ⬜ |

---

## 八、网络/WiFi模拟

| # | 问题 | 当前值 | 目标值 | 方案 | 状态 |
|---|------|--------|--------|------|------|
| 42 | eth0仍可见 | eth0@if96 | 只有wlan0 | macvlan网络 或 Hook网络接口枚举 | ⬜ |
| 43 | WiFi扫描结果 | 空/无 | 周围多个WiFi AP | Hook `WifiManager.getScanResults()` 返回伪造AP列表 | ⬜ |
| 44 | WiFi连接信息 | 无 | 已连接某SSID | Hook `WifiInfo` 返回伪造SSID/BSSID/信号强度 | ⬜ |
| 45 | ARP表 | Docker bridge 172.17.0.1 | 真实路由器IP | Hook `/proc/net/arp` 读取 或 macvlan | ⬜ |

---

## 九、生活痕迹完善

| # | 问题 | 方案 | 状态 |
|---|------|------|------|
| 46 | 通讯录 | provision.sh已有(30-68个) | ✅ 需验证 |
| 47 | 短信记录 | provision.sh里有content insert | ⬜ 需验证 |
| 48 | 通话记录 | 需添加到provision | ⬜ |
| 49 | 安装时间分散 | provision.sh已有(随机30-600天) | ✅ 需验证 |
| 50 | 浏览器历史 | 空 | 预置一些常见网址 | ⬜ |
| 51 | 输入法使用记录 | 空 | 搜狗/百度输入法词库 | ⬜ |
| 52 | 壁纸 | 系统默认 | 预置小米风格 | ⬜ |
| 53 | 锁屏密码/指纹 | 无 | 可选设置PIN | ⬜ |

---

## 十、性能/体验

| # | 问题 | 当前值 | 目标值 | 方案 | 状态 |
|---|------|--------|--------|------|------|
| 54 | FPS | 30 | 60 | Docker参数 redroid_fps=60 | ⬜ |
| 55 | 分辨率 | 未确认 | 1080x2340 (匹配小米10) | Docker参数 | ⬜ |
| 56 | RAM | 需匹配机型 | 8GB (小米10=8GB) | Docker --memory=8g | ⬜ |
| 57 | 摄像头 | /dev/video10,11存在但HAL不识别 | 前后2个摄像头 | Camera HAL适配 | ⬜ |

---

## 十一、推荐安装方案（按顺序执行）

### 第一批：Magisk模块（不需要LSPosed）
```
1. Shamiko v1.2.5 → magisk --install-module shamiko.zip
2. iUnlockerGL v1.1.5 → magisk --install-module iunlockergl.zip
3. Zygisk-Assistant → magisk --install-module zygisk-assistant.zip
4. PlayIntegrityFix → magisk --install-module pif.zip
→ 重启zygote
→ 验证: Momo root检测、GL字符串
```

### 第二批：LSPosed/CPosed + Xposed模块
```
5. CPosed或JingMatrix LSPosed → magisk --install-module lsposed.zip
→ 重启
→ 验证LSPosed Manager可用

6. HideMyAndroid → 装APK + 在LSPosed中激活
   - 配置画像：设备型号、IMEI、传感器、GPS
   - 对抖音等目标APP启用

7. Device Emulator v5.0.2 → 备选，如果HideMyAndroid不够用

8. 我们自研的4个Xposed模块 → 补充HideMyAndroid没覆盖的：
   - com.groupcontrol.signalspoof (SIM/基站/信号)
   - com.groupcontrol.sensornoise (传感器噪声)
   - com.groupcontrol.gpuspoof (GL字符串备选)
   - com.groupcontrol.inputhide (输入设备备选)
```

### 第三批：provision完善
```
9. 更新provision.sh:
   - resetprop清理残留属性(fingerprint/composer/lineage/redroid_*)
   - 通话记录生成
   - 浏览器历史预置
   - 壁纸设置
```

---

## 十二、验证标准

全部完成后，以下检测必须通过：

| 工具 | 目标 |
|------|------|
| **Momo** | "未发现异常" (绿色) |
| **Ruru** | 12/12 Not Found |
| **Applist Detector** | 9/9 Not Found |
| **抖音Badger** | 0失败项，0验证码，无后台风控标记 |
| **安兔兔/鲁大师** | 识别为Xiaomi M2001J2C |
| **设备检测APP** | 所有硬件信息一致 |

---

## 参考链接

- [CPosed (检测绕过增强LSPosed)](https://github.com/ichason/CPosed)
- [JingMatrix/LSPosed](https://github.com/JingMatrix/LSPosed/releases)
- [Shamiko v1.2.5](https://github.com/LSPosed/LSPosed.github.io/releases)
- [iUnlockerGL (GL字符串Magisk模块)](https://github.com/i-Taylo/iUnlockerGL)
- [HideMyAndroid (一站式反检测)](https://modules.lsposed.org/module/com.wowsoftware.hidemyandroid/)
- [Device Emulator v5.0.2](https://modules.lsposed.org/module/dev.device.emulator/)
- [SpoofMyDevice v2.0.2](https://xdaforums.com/t/app-xposed-spoofmydevice-spoof-your-android-device.4783119/)
- [Android Faker](https://github.com/Android1500/AndroidFaker)
- [Zygisk-Assistant](https://github.com/snake-4/Zygisk-Assistant/releases)
- [FakeXposed](https://github.com/sanfengAndroid/FakeXposed)
- [BypassRootCheckPro](https://github.com/gauravssnl/BypassRootCheckPro)
- [XposedFakeLocation (含传感器)](https://github.com/noobexon1/XposedFakeLocation)
