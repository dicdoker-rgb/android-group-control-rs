# Anti-Fingerprint Zygisk Module

对抗字节跳动 flipped SDK 的 Native 层设备指纹检测。

## 两个核心杀器

### 杀器一：Canvas/WebGL 指纹投毒
- Hook `libGLESv2.so` 的 `glReadPixels` 函数
- 在 GPU 渲染结果返回给 APP 之前，随机篡改 1% 像素的 RGB 值（±2）
- 每次读取的 Hash 值都不同，破坏 WebGL 指纹追踪体系
- Mali GPU 和 Adreno GPU 的渲染差异被噪声淹没

### 杀器二：/proc 文件重定向
- Hook `libc.so` 的 `openat` 函数
- 当目标 APP 读取以下文件时重定向到伪造文件：
  - `/proc/self/mountinfo` → 干净的 ext4 挂载信息（无 overlay/docker）
  - `/proc/mounts` → 同上
  - `/proc/self/cgroup` → 真机格式的 cgroup
  - `/proc/1/cgroup` → 同上
- 访问 `/.dockerenv` 时直接返回"文件不存在"
- 只对目标 APP 生效，不影响系统正常运行

## 编译

需要：
1. Android NDK r25+
2. Dobby 预编译库（https://github.com/jmpews/Dobby）
3. Zygisk 头文件（从 Magisk 源码获取 zygisk.hpp）

```bash
# 放置 Dobby
mkdir -p jni/dobby/include jni/dobby/libs/arm64-v8a
# 复制 dobby.h 到 jni/dobby/include/
# 复制 libdobby.a 到 jni/dobby/libs/arm64-v8a/

# 编译
cd jni && ndk-build
```

## 安装

1. 将编译产物和模块文件打包为 zip
2. 通过 Magisk 安装
3. 重启

## 文件结构

```
anti-fingerprint/
├── module.prop
├── post-fs-data.sh          # 生成伪造文件 + 删除 .dockerenv + bind mount SELinux
├── zygisk/
│   ├── arm64-v8a.so          # 编译产物
│   └── armeabi-v7a.so
└── jni/
    ├── main.cpp              # 核心代码
    ├── CMakeLists.txt
    ├── Android.mk
    ├── Application.mk
    └── dobby/                # Dobby hook 框架
        ├── include/dobby.h
        └── libs/arm64-v8a/libdobby.a
```
