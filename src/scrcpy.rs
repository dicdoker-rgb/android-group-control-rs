// src/scrcpy.rs
//
// Pure Rust scrcpy v2/v3 protocol implementation.
// Manages scrcpy-server push, startup, connection (video + control sockets),
// H.264 frame parsing, control message serialization, FPS counting,
// and group-control fan-out.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ── Constants ──────────────────────────────────────────────────────────

const SCRCPY_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const SCRCPY_VERSION: &str = "3.3.4";
const DEVICE_NAME_LEN: usize = 64;
const MAX_FRAME_SIZE: usize = 4 * 1024 * 1024; // 4 MiB sanity limit

// ── Control message types (scrcpy protocol) ────────────────────────────

const MSG_TYPE_INJECT_KEYCODE: u8 = 0;
const MSG_TYPE_INJECT_TEXT: u8 = 1;
const MSG_TYPE_INJECT_TOUCH_EVENT: u8 = 2;
const MSG_TYPE_INJECT_SCROLL_EVENT: u8 = 3;
const MSG_TYPE_BACK_OR_SCREEN_ON: u8 = 4;
const MSG_TYPE_EXPAND_NOTIFICATION_PANEL: u8 = 5;
const MSG_TYPE_EXPAND_SETTINGS_PANEL: u8 = 6;
const MSG_TYPE_COLLAPSE_PANELS: u8 = 7;
const MSG_TYPE_GET_CLIPBOARD: u8 = 8;
const MSG_TYPE_SET_CLIPBOARD: u8 = 9;
const MSG_TYPE_SET_SCREEN_POWER_MODE: u8 = 10;
const MSG_TYPE_ROTATE_DEVICE: u8 = 11;

// Touch action constants (matches Android MotionEvent)
pub const ACTION_DOWN: u8 = 0;
pub const ACTION_UP: u8 = 1;
pub const ACTION_MOVE: u8 = 2;

// ── Video frame PTS flags ──────────────────────────────────────────────

/// Top bit of PTS field: if set, this is a config/codec-specific data packet
const PTS_FLAG_CONFIG: u64 = 1 << 63;
/// Second-highest bit: key frame
const PTS_FLAG_KEY_FRAME: u64 = 1 << 62;
/// Mask to extract the actual PTS value
const PTS_MASK: u64 = !(PTS_FLAG_CONFIG | PTS_FLAG_KEY_FRAME);

// ── FPS counter ────────────────────────────────────────────────────────

/// Thread-safe FPS counter. Call `tick()` on every frame; `fps()` returns
/// the most recent 1-second measurement.
#[derive(Clone)]
pub struct FpsCounter {
    inner: Arc<FpsInner>,
}

struct FpsInner {
    frame_count: AtomicU64,
    last_fps: AtomicU64, // stored as f64 bits
    last_reset: Mutex<Instant>,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(FpsInner {
                frame_count: AtomicU64::new(0),
                last_fps: AtomicU64::new(0),
                last_reset: Mutex::new(Instant::now()),
            }),
        }
    }

    /// Record one frame received. Recalculates FPS every second.
    pub fn tick(&self) {
        let count = self.inner.frame_count.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut last) = self.inner.last_reset.lock() {
            let elapsed = last.elapsed();
            if elapsed >= Duration::from_secs(1) {
                let fps = count as f64 / elapsed.as_secs_f64();
                self.inner
                    .last_fps
                    .store(fps.to_bits(), Ordering::Relaxed);
                self.inner.frame_count.store(0, Ordering::Relaxed);
                *last = Instant::now();
            }
        }
    }

    /// Most recent FPS value (updated every ~1 second).
    pub fn fps(&self) -> f64 {
        f64::from_bits(self.inner.last_fps.load(Ordering::Relaxed))
    }
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Video frame ────────────────────────────────────────────────────────

pub struct VideoFrame {
    /// Presentation timestamp in microseconds (PTS flags stripped).
    pub pts: u64,
    /// Raw H.264 NAL unit(s) for this frame.
    pub data: Vec<u8>,
    /// True if this packet carries codec configuration (SPS/PPS).
    pub is_config: bool,
    /// True if this is a key frame (IDR).
    pub is_key_frame: bool,
}

// ── ScrcpySession ──────────────────────────────────────────────────────

/// A live scrcpy session for one device.
pub struct ScrcpySession {
    pub serial: String,
    pub video_stream: TcpStream,
    pub control_stream: TcpStream,
    pub device_name: String,
    pub width: u16,
    pub height: u16,
    pub codec_id: u32,
    pub fps_counter: FpsCounter,
    server_process: Option<Child>,
}

impl ScrcpySession {
    fn start(serial: &str, server_jar: &Path) -> Result<Self, String> {
        // 1. Push scrcpy-server to device
        push_server(serial, server_jar)?;

        // 2. Set up ADB forward (scrcpy v3: server listens, client connects)
        let local_port = adb_forward(serial, "localabstract:scrcpy")?;

        // 3. Start scrcpy-server process on device
        let server_process = start_server_process(serial)?;

        // 4. Wait for server to start listening
        thread::sleep(Duration::from_secs(4));

        // 5. First connection = video socket
        eprintln!("[scrcpy] connecting video socket to port {local_port}...");
        let video_stream =
            connect_with_retry("127.0.0.1", local_port, Duration::from_secs(10))?;
        video_stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| format!("set read timeout failed: {e}"))?;
        eprintln!("[scrcpy] video socket connected");

        // 6. Read dummy byte (server signals readiness)
        {
            let mut dummy = [0u8; 1];
            let mut stream_ref = &video_stream;
            eprintln!("[scrcpy] waiting for dummy byte...");
            stream_ref
                .read_exact(&mut dummy)
                .map_err(|e| format!("read dummy byte failed: {e}"))?;
            eprintln!("[scrcpy] dummy byte received: 0x{:02x}", dummy[0]);
        }

        // 7. Second connection = control socket
        eprintln!("[scrcpy] connecting control socket...");
        let control_stream =
            connect_with_retry("127.0.0.1", local_port, Duration::from_secs(5))?;
        eprintln!("[scrcpy] control socket connected");

        // 8. Read device name (64 bytes, NUL-padded)
        eprintln!("[scrcpy] reading device name...");
        let device_name = read_device_name(&video_stream)?;
        eprintln!("[scrcpy] device name: {device_name}");

        // 9. Read codec header: 4 bytes codec_id + 4 bytes width + 4 bytes height
        let codec_id: u32;
        let mut actual_width = 720u16;
        let mut actual_height = 1280u16;
        {
            let mut codec_header = [0u8; 12];
            let mut stream_ref = &video_stream;
            stream_ref
                .read_exact(&mut codec_header)
                .map_err(|e| format!("read codec header failed: {e}"))?;
            codec_id = u32::from_be_bytes(codec_header[0..4].try_into().unwrap());
            let w = u32::from_be_bytes(codec_header[4..8].try_into().unwrap()) as u16;
            let h = u32::from_be_bytes(codec_header[8..12].try_into().unwrap()) as u16;
            if w > 0 && h > 0 {
                actual_width = w;
                actual_height = h;
            }
            eprintln!(
                "[scrcpy] codec=0x{codec_id:08x} resolution={actual_width}x{actual_height}"
            );
        }

        // 10. Switch to no timeout for continuous frame reading
        video_stream
            .set_read_timeout(None)
            .map_err(|e| format!("clear read timeout failed: {e}"))?;

        Ok(Self {
            serial: serial.to_string(),
            video_stream,
            control_stream,
            device_name,
            width: actual_width,
            height: actual_height,
            codec_id,
            fps_counter: FpsCounter::new(),
            server_process: Some(server_process),
        })
    }

    // ── Video reading ──────────────────────────────────────────────────

    /// Read one H.264 packet from the video stream.
    ///
    /// Wire format (scrcpy v2/v3):
    ///   [pts_with_flags: u64 BE][packet_size: u32 BE][data: N bytes]
    ///
    /// The top two bits of `pts` carry flags:
    ///   bit 63 = config packet (SPS/PPS)
    ///   bit 62 = key frame
    pub fn read_video_frame(&mut self) -> Result<VideoFrame, String> {
        let mut header = [0u8; 12];
        self.video_stream
            .read_exact(&mut header)
            .map_err(|e| format!("read frame header failed: {e}"))?;

        let pts_raw = u64::from_be_bytes(header[0..8].try_into().unwrap());
        let size = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;

        let is_config = (pts_raw & PTS_FLAG_CONFIG) != 0;
        let is_key_frame = (pts_raw & PTS_FLAG_KEY_FRAME) != 0;
        let pts = pts_raw & PTS_MASK;

        if size > MAX_FRAME_SIZE {
            return Err(format!("frame too large: {size} bytes"));
        }

        let mut data = vec![0u8; size];
        self.video_stream
            .read_exact(&mut data)
            .map_err(|e| format!("read frame data failed: {e}"))?;

        self.fps_counter.tick();

        Ok(VideoFrame {
            pts,
            data,
            is_config,
            is_key_frame,
        })
    }

    // ── Control message serialization ──────────────────────────────────

    /// Inject a touch event.
    ///
    /// Wire format (28 bytes):
    ///   [type:1][action:1][pointerId:8][x:4][y:4][w:2][h:2][pressure:2][actionButton:4][buttons:4]
    pub fn inject_touch(
        &mut self,
        action: u8,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
    ) -> Result<(), String> {
        let mut buf = Vec::with_capacity(32);
        buf.push(MSG_TYPE_INJECT_TOUCH_EVENT);
        buf.push(action);
        buf.extend_from_slice(&0i64.to_be_bytes()); // pointerId = 0 (finger)
        buf.extend_from_slice(&x.to_be_bytes());
        buf.extend_from_slice(&y.to_be_bytes());
        buf.extend_from_slice(&w.to_be_bytes());
        buf.extend_from_slice(&h.to_be_bytes());
        buf.extend_from_slice(&0xffffu16.to_be_bytes()); // pressure = max
        buf.extend_from_slice(&0u32.to_be_bytes()); // actionButton
        buf.extend_from_slice(&0u32.to_be_bytes()); // buttons
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send touch failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Inject a touch event with a specific pointer ID (for multi-touch).
    pub fn inject_touch_finger(
        &mut self,
        action: u8,
        pointer_id: i64,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
        pressure: u16,
    ) -> Result<(), String> {
        let mut buf = Vec::with_capacity(32);
        buf.push(MSG_TYPE_INJECT_TOUCH_EVENT);
        buf.push(action);
        buf.extend_from_slice(&pointer_id.to_be_bytes());
        buf.extend_from_slice(&x.to_be_bytes());
        buf.extend_from_slice(&y.to_be_bytes());
        buf.extend_from_slice(&w.to_be_bytes());
        buf.extend_from_slice(&h.to_be_bytes());
        buf.extend_from_slice(&pressure.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // actionButton
        buf.extend_from_slice(&0u32.to_be_bytes()); // buttons
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send touch failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Inject a key event.
    ///
    /// Wire format (14 bytes):
    ///   [type:1][action:1][keycode:4][repeat:4][metaState:4]
    pub fn inject_keycode(&mut self, action: u8, keycode: u32) -> Result<(), String> {
        self.inject_keycode_meta(action, keycode, 0, 0)
    }

    /// Inject a key event with repeat count and meta state.
    pub fn inject_keycode_meta(
        &mut self,
        action: u8,
        keycode: u32,
        repeat: u32,
        meta_state: u32,
    ) -> Result<(), String> {
        let mut buf = Vec::with_capacity(14);
        buf.push(MSG_TYPE_INJECT_KEYCODE);
        buf.push(action);
        buf.extend_from_slice(&keycode.to_be_bytes());
        buf.extend_from_slice(&repeat.to_be_bytes());
        buf.extend_from_slice(&meta_state.to_be_bytes());
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send keycode failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Inject text (IME composition).
    ///
    /// Wire format: [type:1][length:4][utf8_data:N]
    pub fn inject_text(&mut self, text: &str) -> Result<(), String> {
        let bytes = text.as_bytes();
        let mut buf = Vec::with_capacity(5 + bytes.len());
        buf.push(MSG_TYPE_INJECT_TEXT);
        buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(bytes);
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send text failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Inject a scroll event.
    ///
    /// Wire format:
    ///   [type:1][x:4][y:4][w:2][h:2][scrollX:i32][scrollY:i32][buttons:4]
    pub fn inject_scroll(
        &mut self,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
        scroll_x: i32,
        scroll_y: i32,
    ) -> Result<(), String> {
        let mut buf = Vec::with_capacity(21);
        buf.push(MSG_TYPE_INJECT_SCROLL_EVENT);
        buf.extend_from_slice(&x.to_be_bytes());
        buf.extend_from_slice(&y.to_be_bytes());
        buf.extend_from_slice(&w.to_be_bytes());
        buf.extend_from_slice(&h.to_be_bytes());
        buf.extend_from_slice(&scroll_x.to_be_bytes());
        buf.extend_from_slice(&scroll_y.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // buttons
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send scroll failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Press BACK or turn screen on.
    pub fn back_or_screen_on(&mut self, action: u8) -> Result<(), String> {
        let buf = [MSG_TYPE_BACK_OR_SCREEN_ON, action];
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send back_or_screen_on failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Expand notification panel.
    pub fn expand_notification_panel(&mut self) -> Result<(), String> {
        self.control_stream
            .write_all(&[MSG_TYPE_EXPAND_NOTIFICATION_PANEL])
            .map_err(|e| format!("send expand_notification failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Expand settings panel.
    pub fn expand_settings_panel(&mut self) -> Result<(), String> {
        self.control_stream
            .write_all(&[MSG_TYPE_EXPAND_SETTINGS_PANEL])
            .map_err(|e| format!("send expand_settings failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Collapse all panels.
    pub fn collapse_panels(&mut self) -> Result<(), String> {
        self.control_stream
            .write_all(&[MSG_TYPE_COLLAPSE_PANELS])
            .map_err(|e| format!("send collapse_panels failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Set the device clipboard.
    ///
    /// Wire format: [type:1][sequence:8][paste:1][length:4][utf8_data:N]
    pub fn set_clipboard(&mut self, text: &str, paste: bool) -> Result<(), String> {
        let bytes = text.as_bytes();
        let mut buf = Vec::with_capacity(14 + bytes.len());
        buf.push(MSG_TYPE_SET_CLIPBOARD);
        buf.extend_from_slice(&0u64.to_be_bytes()); // sequence
        buf.push(if paste { 1 } else { 0 });
        buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(bytes);
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send set_clipboard failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Get clipboard content (async response comes on the device message stream).
    pub fn get_clipboard(&mut self, copy_key: u8) -> Result<(), String> {
        let buf = [MSG_TYPE_GET_CLIPBOARD, copy_key];
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send get_clipboard failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Set screen power mode (0=OFF, 2=NORMAL).
    pub fn set_screen_power_mode(&mut self, mode: u8) -> Result<(), String> {
        let buf = [MSG_TYPE_SET_SCREEN_POWER_MODE, mode];
        self.control_stream
            .write_all(&buf)
            .map_err(|e| format!("send set_screen_power_mode failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    /// Rotate the device.
    pub fn rotate_device(&mut self) -> Result<(), String> {
        self.control_stream
            .write_all(&[MSG_TYPE_ROTATE_DEVICE])
            .map_err(|e| format!("send rotate_device failed: {e}"))?;
        self.control_stream
            .flush()
            .map_err(|e| format!("flush failed: {e}"))
    }

    // ── Convenience methods ────────────────────────────────────────────

    /// Tap at (x, y) using the session's own resolution.
    pub fn tap(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.inject_touch(ACTION_DOWN, x, y, self.width, self.height)?;
        self.inject_touch(ACTION_UP, x, y, self.width, self.height)?;
        Ok(())
    }

    /// Long press at (x, y) for the given duration.
    pub fn long_press(&mut self, x: i32, y: i32, duration: Duration) -> Result<(), String> {
        self.inject_touch(ACTION_DOWN, x, y, self.width, self.height)?;
        thread::sleep(duration);
        self.inject_touch(ACTION_UP, x, y, self.width, self.height)?;
        Ok(())
    }

    /// Swipe from (x1,y1) to (x2,y2) over `steps` intermediate MOVE events.
    pub fn swipe(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        steps: u32,
    ) -> Result<(), String> {
        let (w, h) = (self.width, self.height);
        self.inject_touch(ACTION_DOWN, x1, y1, w, h)?;
        let steps = steps.max(1);
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let cx = x1 + ((x2 - x1) as f64 * t) as i32;
            let cy = y1 + ((y2 - y1) as f64 * t) as i32;
            self.inject_touch(ACTION_MOVE, cx, cy, w, h)?;
            thread::sleep(Duration::from_millis(8)); // ~120Hz touch rate
        }
        self.inject_touch(ACTION_UP, x2, y2, w, h)?;
        Ok(())
    }

    /// Key press = DOWN + UP.
    pub fn key_press(&mut self, keycode: u32) -> Result<(), String> {
        self.inject_keycode(ACTION_DOWN, keycode)?;
        self.inject_keycode(ACTION_UP, keycode)?;
        Ok(())
    }

    /// Press HOME (keycode 3).
    pub fn press_home(&mut self) -> Result<(), String> {
        self.key_press(3)
    }

    /// Press BACK (keycode 4).
    pub fn press_back(&mut self) -> Result<(), String> {
        self.key_press(4)
    }

    /// Press RECENTS / app switch (keycode 187).
    pub fn press_recents(&mut self) -> Result<(), String> {
        self.key_press(187)
    }

    fn shutdown(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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

// ── ScrcpyManager ──────────────────────────────────────────────────────

/// Manages all device scrcpy sessions.
pub struct ScrcpyManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<ScrcpySession>>>>,
    server_jar: PathBuf,
}

impl ScrcpyManager {
    pub fn new() -> Self {
        let server_jar = find_scrcpy_server().unwrap_or_else(|| {
            PathBuf::from("/opt/homebrew/Cellar/scrcpy/3.3.4/share/scrcpy/scrcpy-server")
        });
        Self {
            sessions: Mutex::new(HashMap::new()),
            server_jar,
        }
    }

    /// Start (or return existing) scrcpy session for the given device serial.
    pub fn start_session(&self, serial: &str) -> Result<Arc<Mutex<ScrcpySession>>, String> {
        // Return existing session if present
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

    /// Stop and remove the session for the given device.
    pub fn stop_session(&self, serial: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(session) = sessions.remove(serial) {
                if let Ok(mut s) = session.lock() {
                    s.shutdown();
                }
            }
        }
    }

    /// Get an active session by serial.
    pub fn get_session(&self, serial: &str) -> Option<Arc<Mutex<ScrcpySession>>> {
        self.sessions.lock().ok()?.get(serial).cloned()
    }

    /// List all active session serials.
    pub fn active_serials(&self) -> Vec<String> {
        self.sessions
            .lock()
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default()
    }
}

// ── Group control: fan-out dispatcher ──────────────────────────────────

/// Fan-out a single control event to multiple device sessions in parallel.
/// Returns a map of serial -> Result for each device.
pub struct GroupController;

/// Describes a control event to fan-out.
#[derive(Clone, Debug)]
pub enum ControlEvent {
    Tap { x: i32, y: i32 },
    Touch { action: u8, x: i32, y: i32 },
    Swipe { x1: i32, y1: i32, x2: i32, y2: i32, steps: u32 },
    KeyPress { keycode: u32 },
    KeyEvent { action: u8, keycode: u32 },
    Text { text: String },
    Scroll { x: i32, y: i32, scroll_x: i32, scroll_y: i32 },
    SetClipboard { text: String, paste: bool },
    Back,
    Home,
    Recents,
    RotateDevice,
    SetScreenPower { mode: u8 },
}

impl GroupController {
    /// Dispatch `event` to all `serials` using the given manager.
    /// Returns a Vec of (serial, Result) — one per device.
    pub fn dispatch(
        manager: &ScrcpyManager,
        serials: &[String],
        event: &ControlEvent,
    ) -> Vec<(String, Result<(), String>)> {
        let mut results = Vec::with_capacity(serials.len());

        // Collect sessions first so we don't hold the manager lock
        let sessions: Vec<(String, Arc<Mutex<ScrcpySession>>)> = serials
            .iter()
            .filter_map(|s| manager.get_session(s).map(|sess| (s.clone(), sess)))
            .collect();

        // Fan out using scoped threads for true parallelism
        // (avoids requiring tokio runtime in this sync context)
        let event = event.clone();
        std::thread::scope(|scope| {
            let handles: Vec<_> = sessions
                .iter()
                .map(|(serial, session)| {
                    let serial = serial.clone();
                    let event = event.clone();
                    let session = session.clone();
                    scope.spawn(move || {
                        let result = apply_event(&session, &event);
                        (serial, result)
                    })
                })
                .collect();

            for handle in handles {
                match handle.join() {
                    Ok(r) => results.push(r),
                    Err(_) => {} // thread panicked, skip
                }
            }
        });

        // Report serials that had no active session
        for serial in serials {
            if !results.iter().any(|(s, _)| s == serial) {
                results.push((
                    serial.clone(),
                    Err(format!("no active session for {serial}")),
                ));
            }
        }

        results
    }
}

/// Apply a single control event to a session.
fn apply_event(
    session: &Arc<Mutex<ScrcpySession>>,
    event: &ControlEvent,
) -> Result<(), String> {
    let mut s = session
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    match event {
        ControlEvent::Tap { x, y } => s.tap(*x, *y),
        ControlEvent::Touch { action, x, y } => {
            let (w, h) = (s.width, s.height);
            s.inject_touch(*action, *x, *y, w, h)
        }
        ControlEvent::Swipe {
            x1,
            y1,
            x2,
            y2,
            steps,
        } => s.swipe(*x1, *y1, *x2, *y2, *steps),
        ControlEvent::KeyPress { keycode } => s.key_press(*keycode),
        ControlEvent::KeyEvent { action, keycode } => s.inject_keycode(*action, *keycode),
        ControlEvent::Text { text } => s.inject_text(text),
        ControlEvent::Scroll {
            x,
            y,
            scroll_x,
            scroll_y,
        } => {
            let (w, h) = (s.width, s.height);
            s.inject_scroll(*x, *y, w, h, *scroll_x, *scroll_y)
        }
        ControlEvent::SetClipboard { text, paste } => s.set_clipboard(text, *paste),
        ControlEvent::Back => s.press_back(),
        ControlEvent::Home => s.press_home(),
        ControlEvent::Recents => s.press_recents(),
        ControlEvent::RotateDevice => s.rotate_device(),
        ControlEvent::SetScreenPower { mode } => s.set_screen_power_mode(*mode),
    }
}

// ── Helper functions ───────────────────────────────────────────────────

fn find_scrcpy_server() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/Cellar/scrcpy/3.3.4/share/scrcpy/scrcpy-server",
        "/opt/homebrew/share/scrcpy/scrcpy-server",
        "/usr/local/share/scrcpy/scrcpy-server",
        "/usr/share/scrcpy/scrcpy-server",
    ];
    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn push_server(serial: &str, server_jar: &Path) -> Result<(), String> {
    let output = Command::new("adb")
        .args([
            "-s",
            serial,
            "push",
            &server_jar.display().to_string(),
            SCRCPY_SERVER_PATH,
        ])
        .output()
        .map_err(|e| format!("adb push failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "push scrcpy-server failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn adb_forward(serial: &str, remote: &str) -> Result<u16, String> {
    let output = Command::new("adb")
        .args(["-s", serial, "forward", "tcp:0", remote])
        .output()
        .map_err(|e| format!("adb forward failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "set port forward failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let port_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    port_str
        .parse::<u16>()
        .map_err(|_| format!("parse port failed: {port_str}"))
}

fn connect_with_retry(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let addr = format!("{host}:{port}");
    let deadline = Instant::now() + timeout;
    loop {
        match TcpStream::connect(&addr) {
            Ok(stream) => return Ok(stream),
            Err(_) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("connect {addr} failed: {e}")),
        }
    }
}

fn start_server_process(serial: &str) -> Result<Child, String> {
    Command::new("adb")
        .args([
            "-s",
            serial,
            "shell",
            &format!(
                "CLASSPATH={} app_process / com.genymobile.scrcpy.Server {} \
                 tunnel_forward=true video_codec=h264 max_size=720 max_fps=30 \
                 video_encoder=c2.rk.avc.encoder audio=false control=true",
                SCRCPY_SERVER_PATH, SCRCPY_VERSION
            ),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("start scrcpy-server failed: {e}"))
}

fn read_device_name(stream: &TcpStream) -> Result<String, String> {
    let mut buf = [0u8; DEVICE_NAME_LEN];
    let mut stream = stream;
    stream
        .read_exact(&mut buf)
        .map_err(|e| format!("read device name failed: {e}"))?;
    let name = String::from_utf8_lossy(&buf)
        .trim_end_matches('\0')
        .to_string();
    Ok(name)
}
