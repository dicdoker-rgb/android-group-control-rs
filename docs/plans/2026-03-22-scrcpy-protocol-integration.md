# Scrcpy 协议集成 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 scrcpy 协议替代 screencap + input tap，实现 30fps 低延迟画面 + 5ms 输入注入，让群控主板告别卡顿。

**Architecture:** Rust 后端直接管理每台设备的 scrcpy-server 进程，通过 ADB tunnel 建立 video socket（接收 H.264 流）和 control socket（发送二进制输入事件）。视频流通过 WebSocket 转发给浏览器，浏览器用 WebCodecs/Broadway 解码渲染。输入事件从浏览器 WebSocket → Rust → scrcpy control socket，绕过 ADB shell。

**Tech Stack:** Rust (tokio, axum WebSocket), scrcpy-server 3.3.4, H.264 NALU streaming, scrcpy binary control protocol, WebCodecs (前端解码)

**当前环境:**
- scrcpy 3.3.4 已安装，server jar 在 `/opt/homebrew/Cellar/scrcpy/3.3.4/share/scrcpy/scrcpy-server`
- 设备: Redroid 容器 on RK3588, 硬件编码器 `c2.rk.avc.encoder`
- 已有 WhepPreview 组件（WebRTC 播放）可复用手势逻辑

---

## 核心协议说明

### scrcpy-server 通信协议

scrcpy-server 启动后创建两个 socket 连接：

**1. Video Socket（设备→客户端）**
- 先发送 64 字节设备名
- 然后持续发送 H.264 NALU 帧
- 每帧格式: `[pts:8bytes][size:4bytes][nalu_data:N bytes]`
- 首帧包含 SPS/PPS 配置

**2. Control Socket（客户端→设备）**
- 二进制协议，每条消息以 1 字节 type 开头
- Touch: `type(1) + action(1) + pointerId(8) + x(4) + y(4) + w(2) + h(2) + pressure(2) + actionButton(4) + buttons(4)` = 32 bytes
- Keycode: `type(1) + action(1) + keycode(4) + repeat(4) + metaState(4)` = 14 bytes
- Text: `type(1) + len(4) + utf8_bytes(N)`

### 启动 scrcpy-server 的流程

```bash
# 1. 推送 server 到设备
adb -s {serial} push scrcpy-server /data/local/tmp/scrcpy-server.jar

# 2. 创建 ADB tunnel（反向端口转发）
adb -s {serial} reverse localabstract:scrcpy tcp:{local_port}

# 3. 启动 server
adb -s {serial} shell CLASSPATH=/data/local/tmp/scrcpy-server.jar \
  app_process / com.genymobile.scrcpy.Server 3.3.4 \
  tunnel_forward=true video_codec=h264 max_size=720 max_fps=30 \
  video_encoder=c2.rk.avc.encoder audio=false control=true
```

Server 启动后连接到客户端监听的端口，先 video socket，再 control socket。

---

## File Structure

### Rust — New Files
| File | Responsibility |
|------|---------------|
| `src/scrcpy.rs` | scrcpy-server 生命周期管理 + 协议解析 |

### Rust — Modified Files
| File | Changes |
|------|---------|
| `src/lib.rs` | Add `pub mod scrcpy;` |
| `src/bin/web.rs` | 新增 WebSocket 路由 `/api/scrcpy/{serial}` |
| `Cargo.toml` | Add `tokio-tungstenite` (不需要，axum ws 已有) |

### Frontend — New Files
| File | Responsibility |
|------|---------------|
| `frontend/src/hooks/useScrcpy.js` | WebSocket + WebCodecs 解码管理 |
| `frontend/src/components/preview/ScrcpyPlayer.jsx` | H.264 canvas 播放器 + 触控覆盖层 |

### Frontend — Modified Files
| File | Changes |
|------|---------|
| `frontend/src/pages/ControlPage.jsx` | 替换 LiveControl 为 ScrcpyPlayer |

---

## Task 1: Scrcpy Server Manager (`src/scrcpy.rs`)

**Files:**
- Create: `src/scrcpy.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create scrcpy.rs — server 生命周期管理**

```rust
// src/scrcpy.rs
//
// 管理 scrcpy-server 的推送、启动、连接、视频/控制 socket

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SCRCPY_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const SCRCPY_VERSION: &str = "3.3.4";
const DEVICE_NAME_LEN: usize = 64;

/// 单个设备的 scrcpy 会话
pub struct ScrcpySession {
    pub serial: String,
    pub video_stream: TcpStream,    // 读 H.264 帧
    pub control_stream: TcpStream,  // 写控制命令
    pub device_name: String,
    pub width: u16,
    pub height: u16,
    server_process: Option<Child>,
}

/// 管理所有设备的 scrcpy 会话
pub struct ScrcpyManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<ScrcpySession>>>>,
    server_jar: PathBuf,
}

impl ScrcpyManager {
    pub fn new() -> Self {
        // 查找 scrcpy-server jar
        let server_jar = find_scrcpy_server()
            .unwrap_or_else(|| PathBuf::from("/opt/homebrew/Cellar/scrcpy/3.3.4/share/scrcpy/scrcpy-server"));
        Self {
            sessions: Mutex::new(HashMap::new()),
            server_jar,
        }
    }

    /// 为指定设备启动 scrcpy 会话
    pub fn start_session(&self, serial: &str) -> Result<Arc<Mutex<ScrcpySession>>, String> {
        // 如果已有会话且仍然活跃，直接返回
        if let Ok(sessions) = self.sessions.lock() {
            if let Some(session) = sessions.get(serial) {
                return Ok(session.clone());
            }
        }

        let session = ScrcpySession::start(serial, &self.server_jar)?;
        let session = Arc::new(Mutex::new(session));

        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(serial.to_string(), session.clone());
        }

        Ok(session)
    }

    /// 停止指定设备的会话
    pub fn stop_session(&self, serial: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(session) = sessions.remove(serial) {
                if let Ok(mut s) = session.lock() {
                    s.shutdown();
                }
            }
        }
    }

    /// 获取活跃会话
    pub fn get_session(&self, serial: &str) -> Option<Arc<Mutex<ScrcpySession>>> {
        self.sessions.lock().ok()?.get(serial).cloned()
    }
}

impl ScrcpySession {
    fn start(serial: &str, server_jar: &Path) -> Result<Self, String> {
        // 1. 推送 scrcpy-server 到设备
        push_server(serial, server_jar)?;

        // 2. 监听本地端口（让 server 连过来）
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("监听端口失败: {e}"))?;
        let local_port = listener.local_addr()
            .map_err(|e| format!("获取端口失败: {e}"))?
            .port();

        // 设置超时
        listener.set_nonblocking(false)
            .map_err(|e| format!("设置阻塞模式失败: {e}"))?;

        // 3. 设置 ADB 反向端口转发
        adb_reverse(serial, local_port)?;

        // 4. 启动 scrcpy-server
        let server_process = start_server_process(serial)?;

        // 5. 等待 video socket 连接（scrcpy-server 先连 video）
        let video_stream = accept_with_timeout(&listener, Duration::from_secs(10))?;
        video_stream.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("设置读超时失败: {e}"))?;

        // 6. 读取设备名（64 字节）
        let device_name = read_device_name(&video_stream)?;

        // 7. 等待 control socket 连接
        let control_stream = accept_with_timeout(&listener, Duration::from_secs(5))?;

        Ok(Self {
            serial: serial.to_string(),
            video_stream,
            control_stream,
            device_name,
            width: 720,
            height: 1280,
            server_process: Some(server_process),
        })
    }

    /// 读取一帧 H.264 数据
    pub fn read_video_frame(&mut self) -> Result<VideoFrame, String> {
        // scrcpy v3 协议: [pts:8][size:4][data:N]
        let mut header = [0u8; 12];
        self.video_stream.read_exact(&mut header)
            .map_err(|e| format!("读取帧头失败: {e}"))?;

        let pts = u64::from_be_bytes(header[0..8].try_into().unwrap());
        let size = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;

        if size > 4 * 1024 * 1024 {
            return Err("帧数据过大".to_string());
        }

        let mut data = vec![0u8; size];
        self.video_stream.read_exact(&mut data)
            .map_err(|e| format!("读取帧数据失败: {e}"))?;

        Ok(VideoFrame { pts, data })
    }

    /// 发送触摸事件 (scrcpy control protocol)
    pub fn inject_touch(&mut self, action: u8, x: i32, y: i32, w: u16, h: u16) -> Result<(), String> {
        // type=2 (INJECT_TOUCH_EVENT)
        // action: 0=DOWN, 1=UP, 2=MOVE
        let mut buf = Vec::with_capacity(32);
        buf.push(2u8); // type: INJECT_TOUCH_EVENT
        buf.push(action);
        buf.extend_from_slice(&0i64.to_be_bytes()); // pointerId = 0 (finger)
        buf.extend_from_slice(&x.to_be_bytes());
        buf.extend_from_slice(&y.to_be_bytes());
        buf.extend_from_slice(&w.to_be_bytes());
        buf.extend_from_slice(&h.to_be_bytes());
        buf.extend_from_slice(&0xffffu16.to_be_bytes()); // pressure = max
        buf.extend_from_slice(&0u32.to_be_bytes()); // actionButton
        buf.extend_from_slice(&0u32.to_be_bytes()); // buttons
        self.control_stream.write_all(&buf)
            .map_err(|e| format!("发送触摸失败: {e}"))?;
        self.control_stream.flush()
            .map_err(|e| format!("flush 失败: {e}"))
    }

    /// 发送按键事件
    pub fn inject_keycode(&mut self, action: u8, keycode: u32) -> Result<(), String> {
        // type=0 (INJECT_KEYCODE)
        let mut buf = Vec::with_capacity(14);
        buf.push(0u8); // type: INJECT_KEYCODE
        buf.push(action); // 0=DOWN, 1=UP
        buf.extend_from_slice(&keycode.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // repeat
        buf.extend_from_slice(&0u32.to_be_bytes()); // metaState
        self.control_stream.write_all(&buf)
            .map_err(|e| format!("发送按键失败: {e}"))?;
        self.control_stream.flush()
            .map_err(|e| format!("flush 失败: {e}"))
    }

    /// 发送文本
    pub fn inject_text(&mut self, text: &str) -> Result<(), String> {
        // type=1 (INJECT_TEXT)
        let bytes = text.as_bytes();
        let mut buf = Vec::with_capacity(5 + bytes.len());
        buf.push(1u8); // type: INJECT_TEXT
        buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(bytes);
        self.control_stream.write_all(&buf)
            .map_err(|e| format!("发送文本失败: {e}"))?;
        self.control_stream.flush()
            .map_err(|e| format!("flush 失败: {e}"))
    }

    /// 便捷方法: tap
    pub fn tap(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.inject_touch(0, x, y, self.width, self.height)?; // DOWN
        self.inject_touch(1, x, y, self.width, self.height)?; // UP
        Ok(())
    }

    /// 便捷方法: key press
    pub fn key_press(&mut self, keycode: u32) -> Result<(), String> {
        self.inject_keycode(0, keycode)?; // DOWN
        self.inject_keycode(1, keycode)?; // UP
        Ok(())
    }

    fn shutdown(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // 移除反向转发
        let _ = Command::new("adb")
            .args(["-s", &self.serial, "reverse", "--remove-all"])
            .output();
    }
}

impl Drop for ScrcpySession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct VideoFrame {
    pub pts: u64,
    pub data: Vec<u8>,
}

// --- Helper functions ---

fn find_scrcpy_server() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/Cellar/scrcpy/3.3.4/share/scrcpy/scrcpy-server",
        "/opt/homebrew/share/scrcpy/scrcpy-server",
        "/usr/local/share/scrcpy/scrcpy-server",
        "/usr/share/scrcpy/scrcpy-server",
    ];
    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() { return Some(p); }
    }
    // Try `scrcpy --version` to find installation
    None
}

fn push_server(serial: &str, server_jar: &Path) -> Result<(), String> {
    let output = Command::new("adb")
        .args(["-s", serial, "push", &server_jar.display().to_string(), SCRCPY_SERVER_PATH])
        .output()
        .map_err(|e| format!("adb push 失败: {e}"))?;
    if !output.status.success() {
        return Err(format!("推送 scrcpy-server 失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn adb_reverse(serial: &str, local_port: u16) -> Result<(), String> {
    let output = Command::new("adb")
        .args(["-s", serial, "reverse", "localabstract:scrcpy", &format!("tcp:{local_port}")])
        .output()
        .map_err(|e| format!("adb reverse 失败: {e}"))?;
    if !output.status.success() {
        return Err(format!("设置反向转发失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn start_server_process(serial: &str) -> Result<Child, String> {
    Command::new("adb")
        .args(["-s", serial, "shell",
            &format!(
                "CLASSPATH={} app_process / com.genymobile.scrcpy.Server {} \
                 tunnel_forward=true video_codec=h264 max_size=720 max_fps=30 \
                 video_encoder=c2.rk.avc.encoder audio=false control=true \
                 send_frame_meta=true raw_stream=false",
                SCRCPY_SERVER_PATH, SCRCPY_VERSION
            )
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 scrcpy-server 失败: {e}"))
}

fn accept_with_timeout(listener: &TcpListener, timeout: Duration) -> Result<TcpStream, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                return Ok(stream);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err("等待 scrcpy-server 连接超时".to_string());
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("accept 失败: {e}")),
        }
    }
}

fn read_device_name(stream: &TcpStream) -> Result<String, String> {
    let mut buf = [0u8; DEVICE_NAME_LEN];
    let mut stream = stream;
    stream.read_exact(&mut buf)
        .map_err(|e| format!("读取设备名失败: {e}"))?;
    let name = String::from_utf8_lossy(&buf)
        .trim_end_matches('\0')
        .to_string();
    Ok(name)
}
```

- [ ] **Step 2: Add to lib.rs**

Add `pub mod scrcpy;` to `src/lib.rs`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: pass

- [ ] **Step 4: Commit**

```bash
git add src/scrcpy.rs src/lib.rs
git commit -m "feat: add scrcpy protocol module — server lifecycle, video/control sockets"
```

---

## Task 2: WebSocket Video Bridge in web.rs

**Files:**
- Modify: `src/bin/web.rs`

- [ ] **Step 1: Add ScrcpyManager to AppState**

```rust
use android_group_control_rs::scrcpy::ScrcpyManager;

// In AppState:
scrcpy: ScrcpyManager,

// In main():
scrcpy: ScrcpyManager::new(),
```

- [ ] **Step 2: Add WebSocket route**

```rust
.route("/api/scrcpy/{serial}", get(ws_scrcpy))
```

- [ ] **Step 3: Implement ws_scrcpy handler**

This is the core — it bridges scrcpy's TCP video stream to browser WebSocket, and browser input events to scrcpy's control socket:

```rust
async fn ws_scrcpy(
    Path(serial): Path<String>,
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let serial = serial.trim().to_string();
    ws.on_upgrade(move |socket| handle_scrcpy_session(socket, serial, state))
}

async fn handle_scrcpy_session(socket: WebSocket, serial: String, state: Arc<AppState>) {
    use axum::extract::ws::Message;
    use tokio::sync::mpsc;

    // Start scrcpy session
    let session = match state.scrcpy.start_session(&serial) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[scrcpy-ws] 启动失败 {serial}: {e}");
            let _ = socket.close().await;
            return;
        }
    };

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(4);

    // Spawn video reader thread (blocking I/O → tokio channel)
    let video_session = session.clone();
    let video_serial = serial.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            let mut s = match video_session.lock() {
                Ok(s) => s,
                Err(_) => break,
            };
            match s.read_video_frame() {
                Ok(frame) => {
                    if frame_tx.blocking_send(frame.data).is_err() {
                        break; // WebSocket closed
                    }
                }
                Err(e) => {
                    eprintln!("[scrcpy-ws] video read error {video_serial}: {e}");
                    break;
                }
            }
        }
    });

    // Forward video frames to WebSocket (as binary messages)
    let send_task = tokio::spawn(async move {
        while let Some(frame_data) = frame_rx.recv().await {
            if ws_tx.send(Message::Binary(frame_data.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive control messages from WebSocket
    let control_session = session.clone();
    let control_serial = serial.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            let Message::Text(text) = msg else { continue };
            let Ok(payload) = serde_json::from_str::<Value>(&text.to_string()) else { continue };
            let event_type = payload.get("type").and_then(Value::as_str).unwrap_or("");

            let result = {
                let mut s = match control_session.lock() {
                    Ok(s) => s,
                    Err(_) => break,
                };
                match event_type {
                    "tap" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        s.tap(x, y)
                    }
                    "touch_down" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        s.inject_touch(0, x, y, s.width, s.height)
                    }
                    "touch_move" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        s.inject_touch(2, x, y, s.width, s.height)
                    }
                    "touch_up" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        s.inject_touch(1, x, y, s.width, s.height)
                    }
                    "keyevent" => {
                        let keycode = payload.get("keycode").and_then(Value::as_u64).unwrap_or(0) as u32;
                        s.key_press(keycode)
                    }
                    "text" => {
                        let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
                        s.inject_text(text)
                    }
                    _ => Ok(()),
                }
            };

            if let Err(e) = result {
                eprintln!("[scrcpy-ws] control error {control_serial}: {e}");
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Cleanup
    state.scrcpy.stop_session(&serial);
}
```

Note: need `use futures::StreamExt;` for `ws_rx.next()`. Add `futures` to Cargo.toml.

- [ ] **Step 4: Add futures dependency to Cargo.toml**

```toml
futures = "0.3"
```

- [ ] **Step 5: Add needed imports to web.rs**

```rust
use futures::StreamExt;  // for ws_rx.next()
use futures::SinkExt;    // for ws_tx.send()
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check --bin web`

- [ ] **Step 7: Commit**

```bash
git add src/bin/web.rs Cargo.toml
git commit -m "feat: add WebSocket scrcpy bridge — H.264 streaming + control injection"
```

---

## Task 3: Frontend ScrcpyPlayer Component

**Files:**
- Create: `frontend/src/hooks/useScrcpy.js`
- Create: `frontend/src/components/preview/ScrcpyPlayer.jsx`

- [ ] **Step 1: Create useScrcpy.js**

WebSocket hook that receives H.264 binary frames and decodes with WebCodecs:

```javascript
// frontend/src/hooks/useScrcpy.js
import { useState, useRef, useCallback, useEffect } from "react";
import { API_BASE } from "../api";

export function useScrcpy(serial) {
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState(null);
  const wsRef = useRef(null);
  const decoderRef = useRef(null);
  const canvasRef = useRef(null);
  const onFrameRef = useRef(null);

  const wsUrl = serial
    ? `${API_BASE.replace("http", "ws")}/api/scrcpy/${serial}`
    : null;

  const connect = useCallback((canvas) => {
    if (!wsUrl || wsRef.current) return;
    canvasRef.current = canvas;

    // Initialize WebCodecs VideoDecoder
    const decoder = new VideoDecoder({
      output: (frame) => {
        if (canvasRef.current) {
          const ctx = canvasRef.current.getContext("2d");
          canvasRef.current.width = frame.displayWidth;
          canvasRef.current.height = frame.displayHeight;
          ctx.drawImage(frame, 0, 0);
          frame.close();
        }
      },
      error: (e) => {
        console.error("[scrcpy] decode error:", e);
        setError(e.message);
      },
    });
    decoder.configure({
      codec: "avc1.42001f", // H.264 Baseline
      optimizeForLatency: true,
    });
    decoderRef.current = decoder;

    // Open WebSocket
    const ws = new WebSocket(wsUrl);
    ws.binaryType = "arraybuffer";

    let isFirstFrame = true;
    ws.onopen = () => { setConnected(true); setError(null); };
    ws.onclose = () => { setConnected(false); wsRef.current = null; };
    ws.onerror = () => { setError("连接失败"); ws.close(); };

    ws.onmessage = (event) => {
      if (!(event.data instanceof ArrayBuffer)) return;
      const data = new Uint8Array(event.data);

      // Feed H.264 NALU to decoder
      const chunk = new EncodedVideoChunk({
        type: isFirstFrame ? "key" : "delta",
        timestamp: 0,
        data: data,
      });
      try {
        decoder.decode(chunk);
        isFirstFrame = false;
      } catch (e) {
        // If decode fails, next keyframe will fix it
        isFirstFrame = true;
      }
    };

    wsRef.current = ws;
  }, [wsUrl]);

  const disconnect = useCallback(() => {
    if (wsRef.current) { wsRef.current.close(); wsRef.current = null; }
    if (decoderRef.current) {
      try { decoderRef.current.close(); } catch {}
      decoderRef.current = null;
    }
    setConnected(false);
  }, []);

  // Send control events through the same WebSocket
  const send = useCallback((event) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify(event));
  }, []);

  const tap = useCallback((x, y) => send({ type: "tap", x, y }), [send]);

  const touchDown = useCallback((x, y) =>
    send({ type: "touch_down", x, y }), [send]);
  const touchMove = useCallback((x, y) =>
    send({ type: "touch_move", x, y }), [send]);
  const touchUp = useCallback((x, y) =>
    send({ type: "touch_up", x, y }), [send]);

  const keyevent = useCallback((keycode) =>
    send({ type: "keyevent", keycode }), [send]);
  const text = useCallback((t) =>
    send({ type: "text", text: t }), [send]);

  useEffect(() => { return () => disconnect(); }, [disconnect]);

  return {
    connected, error, connect, disconnect,
    tap, touchDown, touchMove, touchUp, keyevent, text,
  };
}
```

- [ ] **Step 2: Create ScrcpyPlayer.jsx**

```jsx
// frontend/src/components/preview/ScrcpyPlayer.jsx
import { useRef, useState, useEffect, useCallback } from "react";
import { useScrcpy } from "../../hooks/useScrcpy";

const TAP_THRESHOLD_PX = 10;
const TAP_THRESHOLD_MS = 300;
const LONGPRESS_MS = 500;

export function ScrcpyPlayer({ serial, onKeyEvent, className = "" }) {
  const canvasRef = useRef(null);
  const scrcpy = useScrcpy(serial);
  const [gesture, setGesture] = useState(null);
  const longpressTimer = useRef(null);

  // Auto-connect when serial changes
  useEffect(() => {
    if (serial && canvasRef.current) {
      scrcpy.connect(canvasRef.current);
    }
    return () => scrcpy.disconnect();
  }, [serial]);

  function toDeviceCoords(e) {
    const canvas = canvasRef.current;
    if (!canvas || !canvas.width || !canvas.height) return null;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = Math.round((e.clientX - rect.left) * scaleX);
    const y = Math.round((e.clientY - rect.top) * scaleY);
    if (x < 0 || y < 0 || x > canvas.width || y > canvas.height) return null;
    return { x, y, cx: e.clientX - rect.left, cy: e.clientY - rect.top };
  }

  function handleMouseDown(e) {
    e.preventDefault();
    const pt = toDeviceCoords(e);
    if (!pt) return;
    scrcpy.touchDown(pt.x, pt.y);
    setGesture({ start: pt, current: pt, startTime: Date.now() });
    longpressTimer.current = setTimeout(() => {
      // Long press: keep finger down, don't send UP
      setGesture(null);
    }, LONGPRESS_MS);
  }

  function handleMouseMove(e) {
    if (!gesture) return;
    const pt = toDeviceCoords(e);
    if (!pt) return;
    const dist = Math.hypot(pt.x - gesture.start.x, pt.y - gesture.start.y);
    if (dist > TAP_THRESHOLD_PX && longpressTimer.current) {
      clearTimeout(longpressTimer.current);
      longpressTimer.current = null;
    }
    scrcpy.touchMove(pt.x, pt.y);
    setGesture(g => g ? { ...g, current: pt } : null);
  }

  function handleMouseUp(e) {
    if (longpressTimer.current) {
      clearTimeout(longpressTimer.current);
      longpressTimer.current = null;
    }
    const pt = toDeviceCoords(e) || gesture?.current;
    if (pt) scrcpy.touchUp(pt.x, pt.y);
    setGesture(null);
  }

  useEffect(() => {
    if (!gesture) return;
    const onMove = (e) => handleMouseMove(e);
    const onUp = (e) => handleMouseUp(e);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [gesture]);

  return (
    <div className={`scrcpy-player ${className}`}>
      <div className="scrcpy-status">
        <span>{scrcpy.connected ? "已连接" : scrcpy.error || "未连接"}</span>
        {!scrcpy.connected && (
          <button className="small secondary" onClick={() => canvasRef.current && scrcpy.connect(canvasRef.current)}>
            连接
          </button>
        )}
      </div>
      <div
        className="scrcpy-canvas-wrapper"
        onMouseDown={handleMouseDown}
        style={{ cursor: "crosshair", position: "relative" }}
      >
        <canvas
          ref={canvasRef}
          style={{ width: "100%", height: "100%", objectFit: "contain" }}
        />
        {gesture && (
          <div className="gesture-dot"
            style={{
              position: "absolute",
              left: gesture.current.cx - 8,
              top: gesture.current.cy - 8,
              width: 16, height: 16,
              borderRadius: "50%",
              background: "rgba(59,130,246,0.5)",
              border: "2px solid rgba(59,130,246,0.8)",
              pointerEvents: "none",
            }}
          />
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/hooks/useScrcpy.js frontend/src/components/preview/ScrcpyPlayer.jsx
git commit -m "feat: add ScrcpyPlayer — WebCodecs H.264 decoder + touch control"
```

---

## Task 4: Integrate ScrcpyPlayer into ControlPage

**Files:**
- Modify: `frontend/src/pages/ControlPage.jsx`

- [ ] **Step 1: Import ScrcpyPlayer and replace LiveControl**

In ControlPage, add ScrcpyPlayer as the primary control mode for "实时操控":
- Import `ScrcpyPlayer` from `../components/preview/ScrcpyPlayer`
- In the "实时操控" tab, replace `LiveControl` with `ScrcpyPlayer`
- Keep common key buttons (Home, Back, Recent) using `scrcpy.keyevent(keycode)`:
  - Home = 3, Back = 4, Recent = 187, Power = 26

- [ ] **Step 2: Add sync mode support**

For sync mode (operating multiple devices simultaneously):
- When sync is ON, each user touch event is sent to multiple WebSocket connections
- Create one `useScrcpy` per selected device, show the focused device's canvas
- Forward touch events from the focused device to all other connections

- [ ] **Step 3: Verify frontend builds**

Run: `cd frontend && npx vite build`

- [ ] **Step 4: Commit**

```bash
git add frontend/src/pages/ControlPage.jsx
git commit -m "feat: integrate ScrcpyPlayer as primary control mode"
```

---

## Task 5: End-to-End Verification

- [ ] **Step 1: Build and start**

```bash
cd /path/to/project
cargo build --bin web
cargo run --bin web &
cd frontend && npx vite &
```

- [ ] **Step 2: Verify scrcpy session lifecycle**

```bash
# Test API endpoint
curl -v http://localhost:8876/api/scrcpy/192.168.31.9:5555
# Should upgrade to WebSocket
```

- [ ] **Step 3: Test in browser**

1. Open `http://localhost:5173`
2. Go to 控制 page
3. Select a device, switch to 实时操控
4. Verify: 30fps H.264 video appears in canvas
5. Click on canvas → device responds instantly
6. Swipe on canvas → device swipes
7. Type text → device receives text

- [ ] **Step 4: Performance comparison**

Old (screencap): ~1 fps, ~500ms input latency
New (scrcpy): ~30 fps, ~30ms input latency

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: scrcpy protocol integration complete — 30fps video + 5ms input"
```
