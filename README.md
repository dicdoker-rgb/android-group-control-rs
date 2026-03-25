# Rust 安卓群控台

原生桌面版群控工具，基于 `egui/eframe`，适合在 macOS 上直接运行。

## 功能

- ADB 设备扫描与状态展示
- 勾选多台设备批量执行动作
- 保存 / 删除设备分组
- 快捷按键、文本输入、坐标点击、滑动
- 启动 / 停止 App
- 批量安装 APK
- 批量执行 ADB Shell
- 截图保存到本地
- 已安装 `scrcpy` 时可直接投屏

## 运行

```bash
cd /Users/showme/Desktop/小说/android-group-control-rs
cargo run
```

首次编译会下载 Rust GUI 依赖，时间会稍长一点。

也可以直接运行：

```bash
cd /Users/showme/Desktop/小说/android-group-control-rs
./start.command
```
