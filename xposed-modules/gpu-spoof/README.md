# GPU Spoof - LSPosed/Xposed Module

An LSPosed module that spoofs GPU information (vendor, renderer, GL version) for Android devices running in Redroid containers on RK3588 hardware.

## Purpose

When apps query GPU information via OpenGL ES or EGL APIs, this module intercepts the calls and returns GPU info matching the spoofed device profile. For example, if the device is configured to appear as a Xiaomi 13 (Snapdragon 8 Gen 2), queries will return "Adreno (TM) 740" instead of the real "Mali-G610".

## How it works

The module hooks three API surfaces:

- **GLES20.glGetString()** - Intercepts GL_VENDOR, GL_RENDERER, and GL_VERSION queries
- **EGL14.eglQueryString()** - Intercepts EGL_VENDOR queries
- **SystemProperties.get()** - Intercepts `ro.hardware.egl` property reads

GPU profile selection is driven by the `ro.board.platform` system property, which should be set to match the target device (e.g., `kalama` for Snapdragon 8 Gen 2).

## Supported platforms

| ro.board.platform | GPU returned              |
|-------------------|---------------------------|
| kona / sm8250     | Adreno (TM) 650           |
| lahaina           | Adreno (TM) 660           |
| taro              | Adreno (TM) 730           |
| kalama            | Adreno (TM) 740           |
| pineapple         | Adreno (TM) 750           |
| sm7150            | Adreno (TM) 618           |
| mt6895            | Mali-G610 MC6             |
| mt6985            | Mali-G715-Immortalis MC11 |

## Scope

Targets the following packages by default:
- `android` (system)
- `com.zhiliaoapp.musically` (TikTok international)
- `com.ss.android.ugc.trill` (TikTok Lite)
- `com.ss.android.ugc.aweme` (Douyin)

## Building

Open the project in Android Studio or build with Gradle. The Xposed API dependency (`de.robv.android.xposed:api`) must be added as `compileOnly` in `build.gradle`.

## Installation

1. Build the APK
2. Install on the Redroid container
3. Enable the module in LSPosed Manager
4. Select target apps in the scope settings
5. Reboot the container
