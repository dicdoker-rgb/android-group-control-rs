# 群控主板 — 集群管控升级设计

## 背景

当前群控主板是一个 ADB 命令批量发送器，缺乏：
- 宿主机/容器生命周期管理
- 自动设备连接和断线重连
- 实时画面互动操控
- 同步镜像操控
- 脚本录制回放
- 宿主机资源监控

目标：升级为完整的云手机集群控制平台。

## 约束

- 宿主机：Orange Pi 5 Plus (RK3588)，会扩展到多台
- 容器：cnflysky/redroid-rk3588:lineage-20
- 通信：先用 SSH（sshpass + Command），后续可演进 agent
- 容器配置：固定模板（GPU host、720x1280、1.5G 内存、0.5 核、30fps、memfd）
- Tauri 打包：所有功能必须在 .app 内正常工作

## 架构总览

```
Tauri Shell (.app)
  └─ Web API (web binary, sidecar)
       ├─ DeviceManager    — ADB 设备操作（现有）
       ├─ HostManager      — 新增：SSH 宿主机通道
       ├─ VmManager        — 新增：容器生命周期
       ├─ AutoConnector    — 新增：自动连接/重连
       ├─ HostMonitor      — 新增：资源监控采集
       └─ InteractiveRelay — 新增：实时操控中继

React 前端
  ├─ DevicesPage    — 设备墙 + 实时预览
  ├─ ControlPage    — 互动操控面板（升级）
  ├─ OpsPage        — 宿主机仪表盘 + 云手机管理（升级）
  ├─ TasksPage      — 任务/录制（现有）
  └─ SettingsPage   — 宿主机配置（新增）
```

---

## 模块 1：宿主机管理 (HostManager)

### 数据结构

`data/hosts.json`:
```json
[{
  "id": "opi-1",
  "name": "香橙派 5 Plus #1",
  "host": "192.168.31.9",
  "ssh_port": 22,
  "user": "root",
  "password": "orangepi",
  "adb_port_start": 5550,
  "adb_port_end": 5599,
  "image": "cnflysky/redroid-rk3588:lineage-20"
}]
```

### 实现

- 新增 `src/host.rs` 模块
- SSH 执行封装：`ssh_exec(host, command) -> Result<String, Error>`
- 使用 `sshpass -p {password} ssh -o StrictHostKeyChecking=no {user}@{host} {command}`
- PATH 问题：通过 Tauri shell 的 `merged_shell_path()` 确保 sshpass 可被找到

### API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/hosts` | GET | 列出所有宿主机 |
| `/api/hosts` | POST | 添加宿主机 |
| `/api/hosts/{id}` | PUT | 编辑宿主机 |
| `/api/hosts/{id}` | DELETE | 删除宿主机 |
| `/api/hosts/{id}/test` | POST | 测试 SSH 连通性 |

---

## 模块 2：云手机生命周期 (VmManager)

### API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/hosts/{id}/vms` | GET | 列出该宿主机上所有容器（含已停止） |
| `/api/hosts/{id}/vms` | POST | 创建 N 台新容器 |
| `/api/hosts/{id}/vms/{name}/start` | POST | 启动 |
| `/api/hosts/{id}/vms/{name}/stop` | POST | 停止（保留数据） |
| `/api/hosts/{id}/vms/{name}/restart` | POST | 重启 |
| `/api/hosts/{id}/vms/{name}` | DELETE | 删除容器+数据 |
| `/api/hosts/{id}/vms/batch` | POST | 批量启停/删除 |

### 创建逻辑

1. 从 `adb_port_start` 扫描已占用端口，找到连续 N 个空闲端口
2. 对每台容器执行（通过 SSH）：
```bash
docker run -d --name redroid-{N} --privileged \
  -p {port}:5555 \
  -v /root/redroid-data-{N}:/data \
  --memory=1536m --cpus=0.5 \
  {image} \
  androidboot.redroid_gpu_mode=host \
  androidboot.redroid_fps=30 \
  androidboot.use_memfd=1 \
  androidboot.redroid_width=720 \
  androidboot.redroid_height=1280 \
  ro.build.characteristics=default
```
3. 等待容器启动完成后自动 `adb connect`

### 删除逻辑

- 停止容器 → 删除容器 → 可选删除数据目录
- 默认保留数据目录（`/root/redroid-data-{N}`），除非用户明确要求清除

---

## 模块 3：自动连接 + 断线重连 (AutoConnector)

### 实现

- Web API 启动时启动后台线程 `auto_connector_loop`
- 每 30 秒一轮：
  1. 读取 `hosts.json` 获取所有宿主机
  2. 对每台宿主机 SSH 执行 `docker ps --format '{{.Names}}\t{{.Ports}}'` 获取运行中容器端口
  3. 对比当前 `adb devices` 列表
  4. 对未连接的设备执行 `adb connect`
  5. 对已断开（offline/unauthorized）的设备先 disconnect 再 reconnect
- 新容器创建后立即触发一次连接
- 连接状态变化通过 SSE 推送到前端

### 数据结构

`AppState` 新增：
```rust
auto_connector: Arc<AutoConnector>,
```

`AutoConnector` 内部维护：
```rust
struct AutoConnector {
    known_devices: RwLock<HashMap<String, DeviceConnectionState>>,
    // serial -> { host_id, port, last_seen, status }
}
```

---

## 模块 4：宿主机监控 (HostMonitor)

### 采集指标

通过 SSH 执行一条合并命令，每 5 秒采集一次：
```bash
echo "CPU:$(top -bn1 | head -3 | tail -1)"
echo "MEM:$(free -m | grep Mem)"
echo "LOAD:$(cat /proc/loadavg)"
echo "TEMP:$(cat /sys/class/thermal/thermal_zone0/temp)"
echo "GPU_FREQ:$(cat /sys/class/devfreq/fb000000.gpu/cur_freq)"
echo "GPU_LOAD:$(cat /sys/class/devfreq/fb000000.gpu/load)"
echo "DOCKER:$(docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}' 2>/dev/null)"
```

### API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/hosts/{id}/stats` | GET | 最近一次采集快照 |
| `/api/hosts/{id}/stats/stream` | GET (SSE) | 实时推送 |

### 前端展示（OpsPage 改造）

- 顶部：宿主机选择器（多台时切换）
- 仪表盘卡片：CPU%、内存%、GPU 频率/负载、温度、负载平均
- 下方：云手机列表表格
  - 列：编号、状态（运行/停止）、CPU%、内存、ADB 连接状态
  - 操作：启动/停止/重启/删除
  - 批量操作：全部启动、全部停止、创建 N 台

---

## 模块 5：实时画面互动操控 (InteractiveRelay)

### 架构

```
用户在前端点击/滑动画面
  → WebSocket 发送坐标事件到 Web API
  → Web API 通过 fast_shell_command 下发到设备
  → 同时通过 preview_hub 刷新画面
```

### 单设备互动

- 前端组件：`InteractivePreview`（已存在，需增强）
- 在设备预览画面上叠加透明交互层
- 捕获 mousedown/mousemove/mouseup 和 touch 事件
- 将画面坐标映射到设备实际分辨率（720x1280）
- 事件类型：
  - 点击：mousedown + mouseup 间距 < 10px 且时间 < 300ms → `input tap x y`
  - 滑动：mousedown → mousemove → mouseup → `input swipe x1 y1 x2 y2 duration`
  - 长按：mousedown 持续 > 500ms → `input swipe x y x y 600`（原地长按）

### API

新增 WebSocket 端点：
```
WS /api/interactive/{serial}
```

消息格式（前端 → 后端）：
```json
{"type": "tap", "x": 360, "y": 640}
{"type": "swipe", "x1": 360, "y1": 800, "x2": 360, "y2": 400, "duration": 300}
{"type": "keyevent", "key": "BACK"}
{"type": "text", "text": "hello"}
```

消息格式（后端 → 前端）：
```json
{"type": "frame", "jpeg_base64": "...", "timestamp": 1234567890}
{"type": "result", "ok": true}
```

### 同步镜像操控

- 选中多台设备后，在"焦点设备"上操作，所有选中设备同步执行
- 前端：用户在一台设备的画面上交互
- 后端：收到事件后广播到所有选中设备的 fast_shell_command
- 坐标映射：所有设备分辨率相同（720x1280），直接转发无需换算

API：
```
WS /api/interactive/sync
```
消息增加 `serials` 字段：
```json
{"type": "tap", "x": 360, "y": 640, "serials": ["192.168.31.9:5555", "192.168.31.9:5552"]}
```

---

## 模块 6：操作录制/回放增强

### 现状

已有 `RecordingTemplate`（步骤列表）和 `run_recording_steps`。但录制过程是手动填参数，不是真正的"录屏录操作"。

### 增强

- 在互动操控模式下增加"录制"按钮
- 录制状态下，所有发送到 WebSocket 的操作事件自动记录为步骤
- 每个步骤记录：动作类型、参数、距上一步的时间间隔
- 录制完成后保存为 RecordingTemplate
- 回放时按时间间隔依次执行，复用现有 `run_recording_steps`

### 数据结构

复用现有 `RecordingStep`，无需修改：
```rust
pub struct RecordingStep {
    pub action: String,    // "tap", "swipe", "keyevent", "text"
    pub params: Value,     // {"x": 360, "y": 640}
    pub delay_ms: u64,     // 距上一步的间隔
}
```

---

## 实现顺序

按依赖关系分 3 个阶段：

### Phase 1 — 基础设施（必须先做）
1. `host.rs` — SSH 执行封装
2. 宿主机 CRUD API
3. 容器生命周期 API
4. 自动连接/重连后台线程
5. 前端：设置页宿主机配置 + OpsPage 云手机管理

### Phase 2 — 监控
6. 宿主机监控采集 + SSE 推送
7. 前端：OpsPage 仪表盘

### Phase 3 — 互动操控
8. WebSocket 互动端点
9. 前端：InteractivePreview 增强（点击/滑动操控）
10. 同步镜像操控
11. 录制模式（录制互动操作为脚本）

---

## 不做的事

- 不做 agent 部署（先用 SSH）
- 不做容器配置自定义（固定模板）
- 不做自动扩缩容
- 不做多用户/权限
- 不做容器内应用管理（用现有的 ADB 批量操作）
