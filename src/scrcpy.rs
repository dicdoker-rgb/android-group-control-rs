// src/scrcpy.rs
//
// 管理 scrcpy-server 的推送、启动、连接、视频/控制 socket

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
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

        // 2. 设置 ADB forward（scrcpy v3: 客户端主动连 server）
        //    server 监听 localabstract:scrcpy，两次连接：第1次=video，第2次=control
        let local_port = adb_forward(serial, "localabstract:scrcpy")?;

        // 3. 启动 scrcpy-server
        let server_process = start_server_process(serial)?;

        // 4. 等 server 启动并开始监听
        thread::sleep(Duration::from_secs(4));

        // 5. 第一次连接 = video socket
        eprintln!("[scrcpy] connecting video socket to port {local_port}...");
        let video_stream = connect_with_retry("127.0.0.1", local_port, Duration::from_secs(10))?;
        video_stream.set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| format!("设置读超时失败: {e}"))?;
        eprintln!("[scrcpy] video socket connected");

        // 6. 读 dummy byte（server 发 1 字节表示准备好了）
        {
            let mut dummy = [0u8; 1];
            let mut stream_ref = &video_stream;
            eprintln!("[scrcpy] waiting for dummy byte...");
            stream_ref.read_exact(&mut dummy)
                .map_err(|e| format!("读取 dummy byte 失败: {e}"))?;
            eprintln!("[scrcpy] dummy byte received: {:02x}", dummy[0]);
        }

        // 7. 第二次连接 = control socket
        eprintln!("[scrcpy] connecting control socket...");
        let control_stream = connect_with_retry("127.0.0.1", local_port, Duration::from_secs(5))?;
        eprintln!("[scrcpy] control socket connected");

        // 8. 读设备名（64 字节）
        eprintln!("[scrcpy] reading device name...");
        let device_name = read_device_name(&video_stream)?;
        eprintln!("[scrcpy] device name: {device_name}");

        // 9. 读 codec info（4 bytes codec_id + 4 bytes width + 4 bytes height）
        let mut actual_width = 720u16;
        let mut actual_height = 1280u16;
        {
            let mut codec_header = [0u8; 12];
            let mut stream_ref = &video_stream;
            stream_ref.read_exact(&mut codec_header)
                .map_err(|e| format!("读取 codec header 失败: {e}"))?;
            let w = u32::from_be_bytes(codec_header[4..8].try_into().unwrap()) as u16;
            let h = u32::from_be_bytes(codec_header[8..12].try_into().unwrap()) as u16;
            if w > 0 && h > 0 {
                actual_width = w;
                actual_height = h;
            }
        }

        // 10. 切换为无超时（持续读帧流）
        video_stream.set_read_timeout(None)
            .map_err(|e| format!("取消读超时失败: {e}"))?;

        Ok(Self {
            serial: serial.to_string(),
            video_stream,
            control_stream,
            device_name,
            width: actual_width,
            height: actual_height,
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
        // 移除端口转发
        let _ = Command::new("adb")
            .args(["-s", &self.serial, "forward", "--remove-all"])
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

fn adb_forward(serial: &str, remote: &str) -> Result<u16, String> {
    // adb forward tcp:0 localabstract:scrcpy — 让 adb 自动分配端口
    let output = Command::new("adb")
        .args(["-s", serial, "forward", "tcp:0", remote])
        .output()
        .map_err(|e| format!("adb forward 失败: {e}"))?;
    if !output.status.success() {
        return Err(format!("设置端口转发失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let port_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    port_str.parse::<u16>().map_err(|_| format!("解析端口失败: {port_str}"))
}

fn connect_with_retry(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let addr = format!("{host}:{port}");
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match TcpStream::connect(&addr) {
            Ok(stream) => return Ok(stream),
            Err(_) if std::time::Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("连接 {addr} 失败: {e}")),
        }
    }
}

fn start_server_process(serial: &str) -> Result<Child, String> {
    // scrcpy v3: tunnel_forward=true means server listens, client connects
    // scrcpy_server_port= not needed, uses localabstract sockets by default
    Command::new("adb")
        .args(["-s", serial, "shell",
            &format!(
                "CLASSPATH={} app_process / com.genymobile.scrcpy.Server {} \
                 tunnel_forward=true video_codec=h264 max_size=720 max_fps=30 \
                 video_encoder=c2.rk.avc.encoder audio=false control=true",
                SCRCPY_SERVER_PATH, SCRCPY_VERSION
            )
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 scrcpy-server 失败: {e}"))
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
