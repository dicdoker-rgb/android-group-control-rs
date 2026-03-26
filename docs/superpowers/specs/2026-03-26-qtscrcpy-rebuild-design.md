# Android 群控系统重构设计 — 基于 QtScrcpy 架构

**日期**：2026-03-26
**状态**：设计阶段（基于 QtScrcpy 源码研究）
**平台**：macOS arm64（开发机）→ 局域网 RK3588 主机（设备宿主）

---

## 1. 目标

将现有 Tauri + React + Rust 群控项目全面重构为 **Qt/QML + QtScrcpyCore + Rust 后端** 架构：

- **画面流畅**：QtScrcpyCore 的 Demuxer→FFmpeg→VideoBuffer 三缓冲→OpenGL/Metal YUV 渲染
- **控制流畅**：scrcpy control socket 协议（触摸 31 字节，<5ms），不再 fork `adb shell input`
- **群控能力**：DeviceObserver 模式，主控设备输入自动扇出到所有从设备
- **任务引擎**：新增工作流编排，支持多设备组并行执行 App 自动化流程
- **UI 风格保留**：用 QML 1:1 还原现有深色主题

---

## 2. 架构总览

```
┌─────────────────────────────────────────────────────────┐
│                     QML UI 层                             │
│  （1:1 还原现有深色主题、卡片布局、侧边栏）                  │
│                                                           │
│  DeviceGrid │ Sidebar │ ActionBar │ Preview │ TaskRunner  │
└──────┬──────┴────┬────┴─────┬────┴────┬────┴─────┬──────┘
       │           │          │         │          │
       │     C++ Bridge 层（QObject，暴露给 QML）     │
       │           │          │         │          │
┌──────┴───────────┴──────────┴─────────┴──────────┴──────┐
│            QtScrcpyCore（C++ 静态库，直接链接）            │
│                                                           │
│  IDeviceManage ─── Device ┬── Server (ADB push/tunnel)   │
│      (单例)               ├── Demuxer (QThread, H.264)   │
│                           ├── Decoder (FFmpeg→YUV420P)   │
│                           ├── Controller (ControlMsg)    │
│                           ├── VideoBuffer (三缓冲)       │
│                           └── FileHandler (APK/文件)     │
│                                                           │
│  GroupController (DeviceObserver 扇出，单例)              │
└──────────────────────────┬──────────────────────────────┘
                           │ FFI (C ABI, JSON 字符串传递)
┌──────────────────────────┴──────────────────────────────┐
│              Rust 后端 (cdylib, libagc_backend.dylib)     │
│                                                           │
│  host.rs          宿主机 SSH 管理、容器生命周期            │
│  auto_connector   后台 ADB 自动连接                       │
│  host_monitor     宿主机 CPU/内存/温度/GPU 监控           │
│  storage          JSON 持久化（groups/profiles/...）      │
│  task_engine/     工作流调度、编排、执行（新增）            │
└─────────────────────────────────────────────────────────┘
```

---

## 3. QtScrcpyCore 集成（基于源码分析）

### 3.1 引入方式

作为 git submodule + CMake `add_subdirectory` 引入。QtScrcpyCore 编译为**静态库**（`add_library(QtScrcpyCore STATIC ...)`），直接链接到主应用。

QtScrcpyCore 自带以下依赖（macOS arm64）：
- FFmpeg 动态库：libavcodec.58, libavformat.58, libavutil.56, libswscale.5
- adb 二进制
- scrcpy-server.jar (v3.3.3)

### 3.2 核心接口（摘自实际源码 QtScrcpyCore.h）

```cpp
namespace qsc {

// 设备观察者 —— 接收帧数据和输入事件的回调接口
class DeviceObserver {
    virtual void onFrame(int width, int height,
        uint8_t* dataY, uint8_t* dataU, uint8_t* dataV,
        int linesizeY, int linesizeU, int linesizeV);
    virtual void updateFPS(quint32 fps);
    virtual void mouseEvent(const QMouseEvent*, const QSize& frameSize, const QSize& showSize);
    virtual void wheelEvent(const QWheelEvent*, const QSize& frameSize, const QSize& showSize);
    virtual void keyEvent(const QKeyEvent*, const QSize& frameSize, const QSize& showSize);
    virtual void postGoBack() / postGoHome() / postGoMenu() / ...;
};

// 单个设备 —— 封装完整的 scrcpy 会话
class IDevice : public QObject {
    void registerDeviceObserver(DeviceObserver*);
    void deRegisterDeviceObserver(DeviceObserver*);
    void setUserData(void*) / void* getUserData();
    bool connectDevice() / void disconnectDevice();
    // 输入事件
    void mouseEvent(...) / wheelEvent(...) / keyEvent(...);
    // 按钮命令
    void postGoBack() / postGoHome() / postPower() / ...;
    // 文件操作
    void pushFileRequest(file, devicePath);
    void installApkRequest(apkFile);
signals:
    void deviceConnected(bool success, serial, deviceName, size);
    void deviceDisconnected(serial);
};

// 设备管理单例
class IDeviceManage : public QObject {
    static IDeviceManage& getInstance();
    bool connectDevice(DeviceParams params);
    bool disconnectDevice(serial);
    QPointer<IDevice> getDevice(serial);
};

}  // namespace qsc
```

### 3.3 DeviceParams 配置（摘自 QtScrcpyCoreDef.h）

```cpp
struct DeviceParams {
    QString serial;                                    // 设备序列号
    QString serverLocalPath;                           // scrcpy-server.jar 本地路径
    quint16 localPort = 27183;                         // 本地端口
    quint16 maxSize = 720;                             // 最大分辨率
    quint32 bitRate = 2000000;                         // 比特率
    quint32 maxFps = 0;                                // 最大帧率（0=不限）
    bool useReverse = true;                            // ADB reverse 优先
    bool stayAwake = false;                            // 保持唤醒
    QString codecName = "";                            // 编码器（空=默认）
    bool renderExpiredFrames = false;                  // 三缓冲丢帧策略
    // ...
};
```

### 3.4 视频管道（实际实现）

```
Android 设备 H.264 编码器
  → TCP socket (ADB reverse/forward tunnel)
    → VideoSocket (QTcpSocket, 移至 Demuxer 线程)
      → Demuxer (QThread::run 阻塞循环)
        读取 12 字节帧头: [PTS+flags 8字节][size 4字节]
        bit63=CONFIG, bit62=KEY_FRAME, bit0-61=PTS
        → FFmpeg av_parser_parse2() 解析 H.264 NAL
        → emit getFrame(AVPacket*) [Qt::DirectConnection]
      → Decoder::push() (在 Demuxer 线程执行)
        → avcodec_send_packet + avcodec_receive_frame
        → YUV420P 写入 VideoBuffer::decodingFrame()
        → offerDecodedFrame() 交换缓冲
        → emit newFrame() [Qt::QueuedConnection → 主线程]
      → Decoder::onNewFrame() (主线程)
        → lock VideoBuffer
        → consumeRenderedFrame() 获取 YUV 帧
        → 回调所有 DeviceObserver::onFrame(Y, U, V 指针)
        → unlock VideoBuffer
      → QYUVOpenGLWidget::updateTextures()
        → glTexSubImage2D() 上传 3 个 GL_LUMINANCE 纹理
        → paintGL() BT.709 YUV→RGB shader 渲染
```

**线程模型**：
| 线程 | 组件 | 职责 |
|------|------|------|
| Demuxer 线程 | Demuxer + VideoSocket | 阻塞读 TCP、H.264 解析、FFmpeg 解码 |
| 主线程 | Device + Controller + Decoder::onNewFrame + QML 渲染 | 输入事件处理、帧回调、OpenGL 渲染 |

### 3.5 控制协议（ControlMsg 序列化格式，摘自 controlmsg.cpp）

| 类型 | 编号 | 格式 |
|------|------|------|
| INJECT_KEYCODE | 0 | 1+1+4+4+4 = 14 字节: type + action + keycode + repeat + metastate |
| INJECT_TEXT | 1 | 1+4+N 字节: type + length(u32be) + UTF-8 text（最大 300 字符） |
| INJECT_TOUCH | 2 | 1+1+8+4+4+2+2+2+4+4 = 32 字节: type + action + pointerId + x + y + w + h + pressure + actionButtons + buttons |
| INJECT_SCROLL | 3 | 1+4+4+2+2+2+2+4 = 21 字节: type + x + y + w + h + hScroll + vScroll + buttons |

PointerId 常量：MOUSE=-1, GENERIC_FINGER=-2, VIRTUAL_MOUSE=-3

### 3.6 群控扇出（GroupController，摘自实际源码）

```cpp
// groupcontroller.cpp — 核心逻辑非常简单
void GroupController::mouseEvent(const QMouseEvent *from, const QSize &frameSize, const QSize &showSize) {
    Q_UNUSED(frameSize);
    for (const auto& serial : m_devices) {
        if (isHost(serial)) continue;    // 跳过主控设备
        auto device = IDeviceManage::getInstance().getDevice(serial);
        if (!device) continue;
        device->mouseEvent(from, getFrameSize(serial), showSize);  // 用目标设备的 frameSize
    }
}

// isHost 通过 getUserData() 获取 VideoForm 指针判断
bool GroupController::isHost(const QString &serial) {
    auto data = IDeviceManage::getInstance().getDevice(serial)->getUserData();
    return data ? static_cast<VideoForm*>(data)->isHost() : true;
}
```

所有命令（goBack/goHome/power/volume/clipboard/installApk/...）都用相同模式扇出。
扇出在**主线程顺序执行**，不做并行。

---

## 4. C++ Bridge 层

薄薄一层 QObject，连接 QML ↔ QtScrcpyCore ↔ Rust FFI。

```cpp
// bridge/DeviceBridge.h
class DeviceBridge : public QObject, public qsc::DeviceObserver {
    Q_OBJECT
    Q_PROPERTY(QVariantList devices READ devices NOTIFY devicesChanged)

public:
    // === QML → QtScrcpyCore ===
    Q_INVOKABLE void connectDevice(const QString &serial);
    Q_INVOKABLE void disconnectDevice(const QString &serial);
    Q_INVOKABLE void setHost(const QString &serial, bool isHost);

    // 控制命令
    Q_INVOKABLE void sendTap(const QString &serial, int x, int y);
    Q_INVOKABLE void sendSwipe(const QString &serial, int x1, int y1, int x2, int y2, int dur);
    Q_INVOKABLE void sendKey(const QString &serial, int keycode);
    Q_INVOKABLE void sendText(const QString &serial, const QString &text);
    Q_INVOKABLE void goBack(const QString &serial);
    Q_INVOKABLE void goHome(const QString &serial);
    Q_INVOKABLE void installApk(const QString &serial, const QString &path);

    // === QML → Rust FFI ===
    Q_INVOKABLE QVariantList getHosts();
    Q_INVOKABLE void createVm(const QString &hostId, int count);
    Q_INVOKABLE void rotateVm(const QString &hostId, const QString &name);
    Q_INVOKABLE void startWorkflow(const QString &workflowId, const QStringList &serials);
    Q_INVOKABLE void stopWorkflow(const QString &executionId);

    // === DeviceObserver 回调 → QML ===
    void onFrame(int w, int h, uint8_t* Y, uint8_t* U, uint8_t* V,
                 int lsY, int lsU, int lsV) override;
    void updateFPS(quint32 fps) override;

signals:
    void devicesChanged();
    void fpsUpdated(const QString &serial, int fps);
    void taskProgress(const QString &taskId, int step, int total, const QString &status);
};
```

```cpp
// bridge/VideoItem.h — QML 中的视频渲染组件
// 继承 QQuickFramebufferObject，内部持有 OpenGL 纹理
// DeviceObserver::onFrame() 直接更新纹理指针（零拷贝，不经过 signal）
// 与 QtScrcpy 的 QYUVOpenGLWidget 相同的 shader 和渲染逻辑
// 适配 QML scene graph 而非 QWidget
class VideoItem : public QQuickFramebufferObject {
    Q_OBJECT
    Q_PROPERTY(QString serial READ serial WRITE setSerial)
    // onFrame() 回调直接在主线程更新纹理，调用 update() 触发重绘
};
```

---

## 5. QML UI 层

### 5.1 组件对应关系

| React 组件（现有） | QML 组件（新） | 说明 |
|---|---|---|
| Sidebar.jsx | Sidebar.qml | 导航栏 |
| DeviceGrid.jsx | DeviceGrid.qml | 设备卡片网格 |
| DeviceCard.jsx | DeviceCard.qml | 单个设备卡片（实时预览 + 状态） |
| GroupBar.jsx | GroupBar.qml | 分组筛选 |
| ActionBar.jsx | ActionBar.qml | 批量操作工具栏 |
| DeviceDetailPanel.jsx | DeviceDetailPanel.qml | 设备详情侧边栏 |
| InteractivePreview.jsx | InteractivePreview.qml | 全屏交互预览（OpenGL 渲染） |
| Header.jsx | Header.qml | 顶部状态栏 |
| HostDashboard.jsx（18行空壳） | HostDashboard.qml | **补全**：宿主机监控 + VM 管理 |

### 5.2 设备控制工具栏（对齐 QtScrcpy ToolForm）

现有 UI 缺少设备控制按钮。参照 QtScrcpy 的 ToolForm，在 DeviceCard 和 InteractivePreview 中增加：

| 按钮 | QtScrcpyCore 方法 | 说明 |
|------|---|---|
| 返回 | `postGoBack()` | Android 返回键 |
| 主页 | `postGoHome()` | Android Home 键 |
| 菜单 | `postGoMenu()` | Android Menu 键 |
| 最近任务 | `postAppSwitch()` | 切换应用 |
| 电源 | `postPower()` | 电源键 |
| 音量+ | `postVolumeUp()` | 音量上 |
| 音量- | `postVolumeDown()` | 音量下 |
| 截图 | `screenshot()` | 截取设备画面 |
| 通知栏 | `expandNotificationPanel()` | 下拉通知 |
| 息屏 | `setDisplayPower(false)` | 关闭屏幕 |
| 群控开关 | `setHost(true/false)` | 设置/取消主控设备 |

这些按钮在 DeviceCard 上以浮层工具栏形式出现（hover 显示），在 InteractivePreview 全屏模式下固定显示在侧边。群控模式下，按钮操作自动扇出到所有从设备。

### 5.3 新增页面

| 页面 | 功能 |
|------|------|
| TaskWorkflow.qml | 工作流编辑器：拖拽编排步骤（点击/滑动/安装/启动/延迟） |
| TaskRunner.qml | 任务执行监控：设备进度、实时日志、失败重试 |

### 5.3 视频渲染

每个 DeviceCard 内嵌一个 `VideoItem`（QQuickFramebufferObject 子类），直接接收 `DeviceObserver::onFrame()` 的 YUV 指针，通过 OpenGL shader 渲染。

InteractivePreview 全屏模式下，鼠标/键盘事件通过 `DeviceBridge` 转发到 `IDevice::mouseEvent/keyEvent`。

### 5.4 深色主题

```qml
// theme/Theme.qml — 定义全局颜色/字体
pragma Singleton
QtObject {
    readonly property color background: "#1a1a2e"
    readonly property color surface: "#16213e"
    readonly property color primary: "#0f3460"
    readonly property color accent: "#e94560"
    readonly property color text: "#eee"
    readonly property color textSecondary: "#aaa"
    readonly property int fontSize: 13
    readonly property int radius: 8
    // ... 从现有 CSS 提取
}
```

---

## 6. Rust 后端（cdylib）

### 6.1 保留模块

| 模块 | 职责 | 变化 |
|------|------|------|
| host.rs | SSH 管理、容器 CRUD、反检测注入 | 改为 C ABI 导出 |
| storage.rs | JSON 持久化 | 改为 C ABI 导出 |
| auto_connector.rs | 后台 ADB 自动发现连接 | 保留，后台线程 |
| host_monitor.rs | 宿主机资源监控 | 移除未使用的 `use Command` |

### 6.2 删除模块

| 模块 | 原因 |
|------|------|
| scrcpy.rs | 由 QtScrcpyCore 替代 |
| preview.rs | 由 QtScrcpyCore VideoBuffer + OpenGL 替代 |
| bezier.rs | 由 QtScrcpyCore InputConvert 替代 |
| src/bin/web.rs | 由 QML + C++ Bridge 替代 |
| src-tauri/ | 不再使用 Tauri |
| frontend/ | 由 QML 替代 |

### 6.3 新增模块：task_engine

```rust
// task_engine/mod.rs — 调度器
pub struct TaskScheduler {
    executions: HashMap<String, WorkflowExecution>,
}

impl TaskScheduler {
    pub fn start_workflow(&mut self, workflow: &Workflow, serials: Vec<String>,
                          control_cb: ControlCallback) -> String;
    pub fn stop_workflow(&mut self, execution_id: &str);
    pub fn get_status(&self, execution_id: &str) -> Option<&WorkflowExecution>;
}
```

```rust
// task_engine/workflow.rs — 工作流定义
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
}

pub enum WorkflowStep {
    // 通过 scrcpy control socket（回调 C++ 层）
    Tap { x: i32, y: i32 },
    Swipe { x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u32 },
    InputText { text: String },
    KeyEvent { keycode: i32 },

    // 通过 ADB 直接执行
    InstallApk { path: String },
    LaunchApp { package: String, activity: String },
    StopApp { package: String },
    Shell { command: String },

    // 流程控制
    Delay { ms: u64 },
    WaitForApp { package: String, timeout_ms: u64 },
}
```

```rust
// task_engine/executor.rs — 每设备执行器
pub struct DeviceExecutor {
    serial: String,
    steps: Vec<WorkflowStep>,
    current_step: usize,
    status: DeviceStatus,
    retry_count: u32,
}
```

**控制命令回调路径**：

工作流中的 Tap/Swipe/Key/Text 步骤需要走 scrcpy control socket，
但 socket 由 QtScrcpyCore (C++) 持有。通过函数指针回调：

```rust
// Rust 侧
type ControlCallback = extern "C" fn(
    serial: *const c_char,
    action: *const c_char,  // "tap", "swipe", "key", "text"
    params_json: *const c_char,
) -> i32;
```

```cpp
// C++ 侧注册回调
extern "C" int control_callback(const char* serial, const char* action, const char* params) {
    auto device = qsc::IDeviceManage::getInstance().getDevice(serial);
    if (!device) return -1;
    // 解析 params JSON，调用 device 对应方法
    // "tap" → 构造 QMouseEvent → device->mouseEvent()
    // "text" → device->postTextInput()
    return 0;
}
```

### 6.4 FFI 接口

Rust 编译为 `libagc_backend.dylib`，所有数据通过 **JSON 字符串** 在 FFI 边界传递（简单安全）：

```rust
#[no_mangle]
pub extern "C" fn rust_load_hosts() -> *mut c_char {
    let hosts = host::load_hosts(&base_dir());
    CString::new(serde_json::to_string(&hosts).unwrap_or_default())
        .unwrap().into_raw()
}

#[no_mangle] pub extern "C" fn rust_free_string(s: *mut c_char);

// 宿主机管理
#[no_mangle] pub extern "C" fn rust_save_host(json: *const c_char) -> i32;
#[no_mangle] pub extern "C" fn rust_test_connection(host_id: *const c_char) -> *mut c_char;
#[no_mangle] pub extern "C" fn rust_list_vms(host_id: *const c_char) -> *mut c_char;
#[no_mangle] pub extern "C" fn rust_create_vms(host_id: *const c_char, count: u32) -> *mut c_char;
#[no_mangle] pub extern "C" fn rust_rotate_vm(host_id: *const c_char, name: *const c_char) -> *mut c_char;

// 任务引擎
#[no_mangle] pub extern "C" fn rust_register_control_callback(cb: ControlCallback);
#[no_mangle] pub extern "C" fn rust_register_progress_callback(cb: ProgressCallback);
#[no_mangle] pub extern "C" fn rust_load_workflows() -> *mut c_char;
#[no_mangle] pub extern "C" fn rust_save_workflow(json: *const c_char) -> i32;
#[no_mangle] pub extern "C" fn rust_start_workflow(workflow_id: *const c_char, serials_json: *const c_char) -> *mut c_char;
#[no_mangle] pub extern "C" fn rust_stop_workflow(execution_id: *const c_char) -> i32;
#[no_mangle] pub extern "C" fn rust_get_execution_status(execution_id: *const c_char) -> *mut c_char;

// 存储 CRUD（同样 JSON 传递）
#[no_mangle] pub extern "C" fn rust_load_groups() -> *mut c_char;
#[no_mangle] pub extern "C" fn rust_save_group(json: *const c_char) -> i32;
// ... 其余 CRUD 同理
```

---

## 7. 项目结构

```
android-group-control/
├── CMakeLists.txt                         # 顶层 CMake
├── third_party/
│   └── QtScrcpy/                          # git submodule（含 QtScrcpyCore 子模块）
│       └── QtScrcpy/QtScrcpyCore/         # 静态库 + FFmpeg + adb + scrcpy-server
├── src/
│   ├── main.cpp                           # Qt 应用入口
│   ├── bridge/
│   │   ├── DeviceBridge.h/cpp             # QML ↔ QtScrcpyCore 桥接
│   │   ├── RustBridge.h/cpp               # QML ↔ Rust FFI 桥接
│   │   ├── VideoItem.h/cpp                # QQuickFramebufferObject YUV 渲染
│   │   └── GroupControlBridge.h/cpp       # 群控状态管理
│   └── qml/
│       ├── main.qml                       # 主窗口
│       ├── Sidebar.qml
│       ├── DeviceGrid.qml
│       ├── DeviceCard.qml
│       ├── GroupBar.qml
│       ├── ActionBar.qml
│       ├── DeviceDetailPanel.qml
│       ├── InteractivePreview.qml
│       ├── HostDashboard.qml
│       ├── TaskWorkflow.qml               # 新增
│       ├── TaskRunner.qml                 # 新增
│       ├── components/
│       │   ├── Badge.qml
│       │   ├── StatCard.qml
│       │   ├── PanelBlock.qml
│       │   └── SegmentedControl.qml
│       └── theme/
│           └── Theme.qml
├── rust_backend/
│   ├── Cargo.toml                         # [lib] crate-type = ["cdylib"]
│   ├── src/
│   │   ├── lib.rs                         # C ABI 导出入口
│   │   ├── ffi.rs                         # FFI 函数
│   │   ├── host.rs                        # 保留
│   │   ├── storage.rs                     # 保留
│   │   ├── auto_connector.rs              # 保留
│   │   ├── host_monitor.rs                # 保留
│   │   └── task_engine/
│   │       ├── mod.rs                     # TaskScheduler
│   │       ├── workflow.rs                # Workflow 定义
│   │       ├── executor.rs                # DeviceExecutor
│   │       └── step.rs                    # WorkflowStep 枚举
│   └── build.rs
├── scripts/                               # 保留
├── kernel-modules/                        # 保留
├── xposed-modules/                        # 保留
├── zygisk-modules/                        # 保留
├── data/                                  # 运行时数据
└── docs/
```

---

## 8. 构建系统

```cmake
cmake_minimum_required(VERSION 3.20)
project(android-group-control)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

# macOS arm64
set(CMAKE_OSX_ARCHITECTURES arm64)

find_package(Qt6 REQUIRED COMPONENTS Quick OpenGL OpenGLWidgets)

# QtScrcpyCore (静态库)
set(QSC_DEPLOY_PATH "${CMAKE_BINARY_DIR}/deploy")
add_subdirectory(third_party/QtScrcpy/QtScrcpy/QtScrcpyCore)

# Rust 后端
add_custom_target(rust_backend ALL
    COMMAND cargo build --release --manifest-path ${CMAKE_SOURCE_DIR}/rust_backend/Cargo.toml
    COMMENT "Building Rust backend"
)

# 主应用
qt_add_executable(${PROJECT_NAME}
    src/main.cpp
    src/bridge/DeviceBridge.cpp
    src/bridge/RustBridge.cpp
    src/bridge/VideoItem.cpp
    src/bridge/GroupControlBridge.cpp
)

target_link_libraries(${PROJECT_NAME} PRIVATE
    Qt6::Quick Qt6::OpenGL Qt6::OpenGLWidgets
    QtScrcpyCore
    ${CMAKE_SOURCE_DIR}/rust_backend/target/release/libagc_backend.dylib
)

qt_add_qml_module(${PROJECT_NAME}
    URI GroupControl
    QML_FILES src/qml/main.qml src/qml/Sidebar.qml src/qml/DeviceGrid.qml
    # ... 其余 QML 文件
)

add_dependencies(${PROJECT_NAME} rust_backend)
```

macOS 打包：`macdeployqt` → `.app` bundle（含 Qt 框架 + FFmpeg dylib + Rust dylib + adb + scrcpy-server）

---

## 9. 迁移计划（6 阶段）

### Phase 1：基础框架（~3天）
- 建立 CMake 项目结构
- 引入 QtScrcpyCore submodule，macOS 编译验证
- Rust 后端改 cdylib，最小 FFI（rust_load_hosts / rust_free_string）
- QML 主窗口 + Sidebar 骨架 + 深色主题

### Phase 2：设备连接 + 视频（~4天）
- DeviceBridge 实现 connectDevice/disconnectDevice
- VideoItem (QQuickFramebufferObject) OpenGL YUV 渲染
- DeviceGrid + DeviceCard 显示实时预览
- **里程碑：单设备 720p@30fps 流畅预览**

### Phase 3：控制 + 群控（~3天）
- InteractivePreview 鼠标/键盘 → IDevice::mouseEvent/keyEvent
- GroupControlBridge + GroupController 群控扇出
- **里程碑：主控设备操作同步到所有从设备**

### Phase 4：宿主机管理（~2天）
- RustBridge FFI 完整桥接（host CRUD、VM 管理）
- HostDashboard QML 完整实现
- **里程碑：UI 上创建/销毁/轮换容器**

### Phase 5：任务引擎（~4天）
- Rust task_engine 模块
- 控制命令回调通路（Rust → C++ → QtScrcpyCore Controller）
- TaskWorkflow QML 编辑器 + TaskRunner 监控
- **里程碑：编排工作流，多设备并行执行**

### Phase 6：功能迁移 + 打磨（~3天）
- 迁移剩余功能：分组、素材、账号、ADB 预设
- macOS .app 打包验证
- 局域网 RK3588 实测

---

## 10. ADB 连接路径（远程 Redroid 容器）

macOS 开发机通过局域网连接 RK3588 宿主机上的 Redroid 容器：

```
macOS (adb client)
  → adb connect 192.168.31.9:5600    # 直接 TCP 连接容器暴露的 ADB 端口
  → QtScrcpyCore 使用该 serial 建立 scrcpy 会话
```

**连接流程**：
1. `auto_connector.rs`（Rust 后端）通过 SSH 扫描宿主机上运行的容器及端口
2. 对每个容器执行 `adb connect host:port`
3. ADB 连接建立后，通知 C++ Bridge 层
4. C++ 调用 `IDeviceManage::getInstance().connectDevice(params)` 建立 scrcpy 会话
5. QtScrcpyCore 通过已连接的 ADB 设备推送 scrcpy-server 并建立 tunnel

**auto_connector 与 QtScrcpyCore 的分工**：
- auto_connector：负责发现容器 + `adb connect`（建立 ADB 连接）
- QtScrcpyCore：负责在已连接的 ADB 设备上启动 scrcpy 会话（推送 server + tunnel + 视频/控制 socket）

---

## 11. 线程模型

| 线程 | 组件 | 职责 |
|------|------|------|
| Qt 主线程 | QML UI + DeviceBridge + Controller + Decoder::onNewFrame | 渲染、输入事件、群控扇出 |
| Demuxer 线程（每设备一个） | Demuxer + VideoSocket + FFmpeg 解码 | 阻塞读 TCP、H.264 解析解码 |
| Rust 后台线程 | auto_connector、host_monitor | SSH 扫描、资源采集 |
| Rust 任务线程（每设备一个） | DeviceExecutor | 工作流步骤顺序执行 |

**Rust 运行时**：任务引擎使用 `std::thread`（不用 Tokio），FFI 回调需跨线程。
**跨线程回调安全**：Rust 任务线程回调 C++ 时，C++ 侧通过 `QMetaObject::invokeMethod(bridge, ..., Qt::QueuedConnection)` 将调用转发到主线程，确保 Qt 对象线程安全。

**回调注册（安全方式）**：
```rust
use std::sync::OnceLock;

static CONTROL_CB: OnceLock<ControlCallback> = OnceLock::new();

#[no_mangle]
pub extern "C" fn rust_register_control_callback(cb: ControlCallback) {
    let _ = CONTROL_CB.set(cb);
}
```

---

## 12. Phase 0：可行性验证（开始前必做）

在正式开发前，执行以下验证：

1. **QtScrcpyCore macOS arm64 编译**：验证 `add_subdirectory(QtScrcpyCore)` 是否能直接编译，记录需要的 patch
2. **API 验证**：确认 `IDevice::mouseEvent()` 等方法的实际参数和行为
3. **单设备连通测试**：用 QtScrcpyCore 连接一台 Redroid 容器，验证视频和控制是否工作
4. **FFmpeg 依赖**：确认 QtScrcpyCore 自带的 FFmpeg arm64 dylib 在 macOS 上可用

---

## 13. 任务引擎容错

| 场景 | 处理方式 |
|------|----------|
| 设备断连 | 标记该设备为 Failed，其他设备继续执行 |
| 步骤失败 | 重试当前步骤（最多 3 次），仍失败则标记设备 Failed |
| 应用崩溃 | 执行状态不持久化（V1），后续可加 checkpoint |
| 取消工作流 | 停止所有设备的执行线程，标记为 Cancelled |
| 可重试错误 | 控制命令超时、ADB 短暂断开 |
| 不可重试错误 | 设备离线、APK 安装失败（签名不匹配） |

---

## 14. 现有页面迁移清单

| React 组件 | 处理方式 | QML 对应 |
|---|---|---|
| DevicesPage.jsx | 迁移（核心） | DeviceGrid.qml |
| ControlPage.jsx | 迁移 → 合并到 InteractivePreview | InteractivePreview.qml |
| OpsPage.jsx | 迁移 | ActionBar.qml + 批量操作对话框 |
| TasksPage.jsx | 重写（任务引擎） | TaskWorkflow.qml + TaskRunner.qml |
| ToolsPage.jsx | 迁移 | ToolsPanel.qml |
| SettingsPage.jsx | 迁移 | SettingsPanel.qml |
| VmTable.jsx | 迁移 | HostDashboard.qml 内嵌 |
| ScrcpyPlayer.jsx | 删除（QtScrcpyCore 替代） | — |
| WhepPreview.jsx | 删除（QtScrcpyCore 替代） | — |
| LiveControl.jsx | 删除（QtScrcpyCore 替代） | — |
| AppContext.jsx | 替换为 C++ 单例 Model | DeviceBridge (QObject singleton) |

---

## 15. 不做的事情

- KeyMap 按键映射（后续需要时添加）
- 音频转发
- 定时任务/Cron（后续 C 阶段）
- 条件分支/截图匹配（后续 C 阶段）
- 用户认证（内部项目）
