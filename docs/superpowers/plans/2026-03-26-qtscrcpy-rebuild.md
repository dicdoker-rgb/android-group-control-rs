# QtScrcpy 群控系统重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 将现有 Tauri+React+Rust 群控项目重构为 Qt/QML+QtScrcpyCore+Rust，实现流畅的视频预览、低延迟控制和工作流任务引擎。

**Architecture:** QtScrcpyCore（C++ 静态库）处理 scrcpy 协议/视频/控制；QML 前端 1:1 还原现有深色 UI；Rust 后端编译为 cdylib 通过 FFI 桥接，负责宿主机管理和任务引擎。

**Tech Stack:** Qt 6 (QML/Quick/OpenGL), QtScrcpyCore, FFmpeg, Rust (cdylib), CMake, macOS arm64

**Spec:** docs/superpowers/specs/2026-03-26-qtscrcpy-rebuild-design.md

---

## File Structure

### New project root: /Users/showme/Desktop/android-group-control/

```
android-group-control/
├── CMakeLists.txt
├── third_party/QtScrcpy/                   # git submodule
├── src/
│   ├── main.cpp
│   ├── bridge/
│   │   ├── DeviceBridge.h / DeviceBridge.cpp
│   │   ├── RustBridge.h / RustBridge.cpp
│   │   ├── VideoItem.h / VideoItem.cpp
│   │   └── GroupControlBridge.h / GroupControlBridge.cpp
│   └── qml/
│       ├── main.qml
│       ├── theme/Theme.qml
│       ├── Sidebar.qml
│       ├── DeviceGrid.qml
│       ├── DeviceCard.qml
│       ├── DeviceToolbar.qml
│       ├── GroupBar.qml
│       ├── ActionBar.qml
│       ├── DeviceDetailPanel.qml
│       ├── InteractivePreview.qml
│       ├── HostDashboard.qml
│       ├── TaskWorkflow.qml
│       ├── TaskRunner.qml
│       ├── ToolsPanel.qml
│       ├── SettingsPanel.qml
│       └── components/{Badge,StatCard,PanelBlock,SegmentedControl}.qml
├── rust_backend/
│   ├── Cargo.toml
│   ├── build.rs
│   └── src/
│       ├── lib.rs
│       ├── ffi.rs
│       ├── host.rs          (from existing)
│       ├── storage.rs       (from existing)
│       ├── auto_connector.rs (from existing)
│       ├── host_monitor.rs   (from existing)
│       └── task_engine/
│           ├── mod.rs
│           ├── workflow.rs
│           ├── executor.rs
│           └── step.rs
└── data/
```

---

## Phase 0: 可行性验证

### Task 0.1: 安装 Qt 6 并验证环境

**Files:** None (system setup)

- [ ] Step 1: 检查是否已安装 Qt 6
- [ ] Step 2: 如果未安装，通过 Homebrew 安装 Qt 6: `brew install qt@6`
- [ ] Step 3: 验证 Qt Quick/OpenGL 模块可用

---

### Task 0.2: 创建新项目并编译 QtScrcpyCore

**Files:**
- Create: CMakeLists.txt
- Create: src/main.cpp

- [ ] Step 1: 创建项目目录并 git init
- [ ] Step 2: 添加 QtScrcpy 为 submodule（含 QtScrcpyCore 子模块，修复 SSH→HTTPS URL）
- [ ] Step 3: 写最小 CMakeLists.txt，add_subdirectory(QtScrcpyCore)
- [ ] Step 4: 写最小 main.cpp 调用 IDeviceManage::getInstance() 验证链接
- [ ] Step 5: cmake + make 编译验证。如果失败，记录 patch 并修复
- [ ] Step 6: Commit: "feat: project scaffold with QtScrcpyCore submodule"

---

### Task 0.3: 单设备连通测试

**Files:**
- Modify: src/main.cpp

- [ ] Step 1: adb connect 192.168.31.9:5600 确认 ADB 连通
- [ ] Step 2: 扩展 main.cpp 调用 IDeviceManage::connectDevice(params) 连接 Redroid 容器
- [ ] Step 3: 确认 scrcpy-server 已部署到正确路径
- [ ] Step 4: 编译运行，验证输出 "Device connected" 和设备尺寸
- [ ] Step 5: Commit: "test: verify QtScrcpyCore connects to Redroid container"

---

## Phase 1: 基础框架 + QML 骨架

### Task 1.1: Qt Quick 项目结构 + 深色主题

**Files:**
- Modify: CMakeLists.txt (添加 Qt6::Quick, qt_add_qml_module)
- Rewrite: src/main.cpp (QGuiApplication + QQmlApplicationEngine)
- Create: src/qml/main.qml
- Create: src/qml/theme/Theme.qml (深色主题 singleton，颜色从现有 CSS 提取)
- Create: src/qml/Sidebar.qml (5 项导航：设备/宿主机/任务/工具/设置)

- [ ] Step 1: 更新 CMakeLists.txt 支持 Qt Quick + QML module
- [ ] Step 2: 重写 main.cpp 为 QGuiApplication + QQmlApplicationEngine
- [ ] Step 3: 创建 Theme.qml 深色主题 singleton
- [ ] Step 4: 创建 main.qml 主窗口骨架（RowLayout: Sidebar + 内容区）
- [ ] Step 5: 创建 Sidebar.qml（5 个导航项，可点击切换）
- [ ] Step 6: 编译运行验证：深色窗口 + 左侧导航
- [ ] Step 7: Commit: "feat: QML skeleton with dark theme and sidebar"

---

### Task 1.2: Rust 后端改 cdylib + 最小 FFI

**Files:**
- Create: rust_backend/Cargo.toml (crate-type = ["cdylib"])
- Create: rust_backend/src/lib.rs
- Create: rust_backend/src/ffi.rs (rust_load_hosts, rust_free_string, rust_init)
- Copy from existing: host.rs, storage.rs, auto_connector.rs, host_monitor.rs
- Modify: CMakeLists.txt (add_custom_target cargo build + 链接 dylib)

- [ ] Step 1: 创建 Cargo.toml with cdylib
- [ ] Step 2: 复制现有 Rust 模块（不复制 scrcpy/preview/bezier/web.rs）
- [ ] Step 3: 创建 lib.rs 声明模块
- [ ] Step 4: 创建 ffi.rs 导出 rust_load_hosts/rust_free_string/rust_init
- [ ] Step 5: cargo build --release 修复编译错误（移除对已删模块的引用）
- [ ] Step 6: CMakeLists.txt 添加 cargo build target + 链接 dylib
- [ ] Step 7: main.cpp 中调用 rust_init() + rust_load_hosts() 验证
- [ ] Step 8: 编译运行验证输出 "Hosts: []"
- [ ] Step 9: Commit: "feat: Rust backend as cdylib with minimal FFI"

---

## Phase 2: 设备连接 + 视频预览

### Task 2.1: DeviceBridge — QML 到 QtScrcpyCore 桥接

**Files:**
- Create: src/bridge/DeviceBridge.h
- Create: src/bridge/DeviceBridge.cpp
- Modify: src/main.cpp (注册 QML context property)
- Modify: CMakeLists.txt

DeviceBridge 是 QObject singleton，暴露给 QML：
- devices 属性（QVariantList）
- connectDevice/disconnectDevice 方法
- 11 个控制命令方法（goBack/goHome/power/volume 等）
- deviceConnected/deviceDisconnected signals

- [ ] Step 1: 创建 DeviceBridge.h 声明
- [ ] Step 2: 实现 DeviceBridge.cpp，调用 IDeviceManage API
- [ ] Step 3: main.cpp 注册为 QML context property
- [ ] Step 4: 更新 CMakeLists.txt
- [ ] Step 5: 编译验证
- [ ] Step 6: Commit: "feat: DeviceBridge connects QML to QtScrcpyCore"

---

### Task 2.2: VideoItem — OpenGL YUV 渲染

**Files:**
- Create: src/bridge/VideoItem.h
- Create: src/bridge/VideoItem.cpp

VideoItem 继承 QQuickFramebufferObject + qsc::DeviceObserver:
- serial 属性绑定设备
- onFrame() 接收 YUV420P 指针，存储到内部缓冲
- Renderer 内部类上传 3 个 GL_LUMINANCE 纹理
- BT.709 fragment shader (从 QtScrcpy QYUVOpenGLWidget 移植)

- [ ] Step 1: 创建 VideoItem.h（QQuickFramebufferObject + DeviceObserver）
- [ ] Step 2: 实现 VideoItem.cpp + Renderer（OpenGL 纹理上传 + shader）
- [ ] Step 3: main.cpp 中 qmlRegisterType 注册
- [ ] Step 4: 编译验证
- [ ] Step 5: Commit: "feat: VideoItem OpenGL YUV renderer for QML"

---

### Task 2.3: DeviceGrid + DeviceCard QML

**Files:**
- Create: src/qml/DeviceGrid.qml
- Create: src/qml/DeviceCard.qml
- Modify: src/qml/main.qml

DeviceCard: VideoItem 显示实时预览 + 设备名称 + 状态点
DeviceGrid: GridView 使用 deviceBridge.devices 作为 model

- [ ] Step 1: 创建 DeviceCard.qml（参照现有 DeviceCard.jsx 布局）
- [ ] Step 2: 创建 DeviceGrid.qml（GridView wrap DeviceCard）
- [ ] Step 3: main.qml 根据 Sidebar 选中项切换内容
- [ ] Step 4: 添加临时连接按钮测试 — 连接 192.168.31.9:5600
- [ ] Step 5: **里程碑验证：单设备 720p@30fps 流畅预览**
- [ ] Step 6: Commit: "feat: DeviceGrid with live video preview"

---

## Phase 3: 控制 + 群控

### Task 3.1: InteractivePreview + DeviceToolbar

**Files:**
- Create: src/qml/InteractivePreview.qml
- Create: src/qml/DeviceToolbar.qml

InteractivePreview: 全屏 VideoItem + MouseArea 捕获鼠标/键盘 → DeviceBridge
DeviceToolbar: 11 个按钮（返回/主页/菜单/最近/电源/音量/截图/通知/息屏/群控）

- [ ] Step 1: 创建 DeviceToolbar.qml（11 个控制按钮）
- [ ] Step 2: 创建 InteractivePreview.qml（全屏预览 + 鼠标/键盘操控）
- [ ] Step 3: DeviceCard 双击打开 InteractivePreview
- [ ] Step 4: 验证：鼠标点击/滑动控制设备
- [ ] Step 5: Commit: "feat: interactive device control with toolbar"

---

### Task 3.2: GroupControlBridge — 群控扇出

**Files:**
- Create: src/bridge/GroupControlBridge.h
- Create: src/bridge/GroupControlBridge.cpp

封装 QtScrcpy GroupController 模式：
- setHost(serial) 注册 GroupController 为 DeviceObserver
- 输入自动扇出到所有非主控设备

- [ ] Step 1: 创建 GroupControlBridge（封装 GroupController 单例逻辑）
- [ ] Step 2: DeviceToolbar 群控按钮连接到 GroupControlBridge
- [ ] Step 3: **里程碑验证：连接 2 台设备，主控操作同步到从设备**
- [ ] Step 4: Commit: "feat: group control fan-out via DeviceObserver"

---

## Phase 4: 宿主机管理

### Task 4.1: RustBridge + HostDashboard

**Files:**
- Create: src/bridge/RustBridge.h
- Create: src/bridge/RustBridge.cpp
- Create: src/qml/HostDashboard.qml
- Modify: rust_backend/src/ffi.rs（导出 host CRUD + VM 管理全部函数）

- [ ] Step 1: ffi.rs 导出 rust_save_host/test_connection/list_vms/create_vms/rotate_vm/delete_vm/get_host_stats
- [ ] Step 2: 创建 RustBridge.h/cpp（调用 FFI，JSON→QVariant 转换）
- [ ] Step 3: 创建 HostDashboard.qml（宿主机列表 + VM 表格 + 资源监控）
- [ ] Step 4: main.qml Sidebar 集成
- [ ] Step 5: **里程碑验证：UI 上添加宿主机、创建/删除 VM**
- [ ] Step 6: Commit: "feat: host dashboard with VM management"

---

## Phase 5: 任务引擎

### Task 5.1: Rust task_engine 模块

**Files:**
- Create: rust_backend/src/task_engine/step.rs
- Create: rust_backend/src/task_engine/workflow.rs
- Create: rust_backend/src/task_engine/executor.rs
- Create: rust_backend/src/task_engine/mod.rs
- Modify: rust_backend/src/lib.rs
- Modify: rust_backend/src/ffi.rs

- [ ] Step 1: 创建 step.rs（WorkflowStep 枚举，serde 支持）
- [ ] Step 2: 创建 workflow.rs（Workflow 结构 + CRUD）
- [ ] Step 3: 创建 executor.rs（DeviceExecutor，每设备一线程，OnceLock 回调）
- [ ] Step 4: 创建 mod.rs（TaskScheduler 管理多 execution）
- [ ] Step 5: ffi.rs 导出 workflow CRUD + start/stop/status + callback 注册
- [ ] Step 6: cargo test 验证
- [ ] Step 7: Commit: "feat: task engine with workflow execution"

---

### Task 5.2: C++ 控制命令回调 + TaskWorkflow/TaskRunner QML

**Files:**
- Modify: src/bridge/RustBridge.cpp（注册 control_callback + progress_callback）
- Create: src/qml/TaskWorkflow.qml
- Create: src/qml/TaskRunner.qml

- [ ] Step 1: RustBridge 中实现 control_callback（QMetaObject::invokeMethod 跨线程）
- [ ] Step 2: 初始化时调用 rust_register_control_callback
- [ ] Step 3: 创建 TaskWorkflow.qml（步骤列表 + 添加/删除/保存/运行）
- [ ] Step 4: 创建 TaskRunner.qml（设备进度表 + 日志 + 停止按钮）
- [ ] Step 5: **里程碑验证：编排工作流，多设备并行执行**
- [ ] Step 6: Commit: "feat: workflow editor and task runner UI"

---

## Phase 6: 功能迁移 + 打包

### Task 6.1: 迁移剩余功能

**Files:**
- Create: src/qml/GroupBar.qml, ActionBar.qml, DeviceDetailPanel.qml, ToolsPanel.qml, SettingsPanel.qml
- Modify: rust_backend/src/ffi.rs（导出分组/素材/账号/预设 CRUD）

- [ ] Step 1: ffi.rs 导出剩余 CRUD
- [ ] Step 2: 实现 GroupBar + ActionBar + DeviceDetailPanel
- [ ] Step 3: 实现 ToolsPanel + SettingsPanel
- [ ] Step 4: Commit: "feat: migrate remaining features from React app"

---

### Task 6.2: macOS .app 打包

**Files:**
- Modify: CMakeLists.txt（MACOSX_BUNDLE + 依赖复制）

- [ ] Step 1: CMakeLists.txt 配置 MACOSX_BUNDLE
- [ ] Step 2: 复制 FFmpeg dylib + Rust dylib + adb + scrcpy-server 到 bundle
- [ ] Step 3: macdeployqt 打包
- [ ] Step 4: **最终验证：RK3588 局域网环境多设备实测**
- [ ] Step 5: Commit: "feat: macOS .app bundle packaging"
