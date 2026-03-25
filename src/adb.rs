use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use image::codecs::jpeg::JpegEncoder;
use image::GenericImageView;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize)]
pub struct RemoteFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub serial: String,
    pub state: String,
    pub product: String,
    pub model_hint: String,
    pub device_hint: String,
    pub transport_id: String,
    pub model: String,
    pub brand: String,
    pub android_version: String,
    #[serde(default)]
    pub custom_name: String,
    #[serde(default)]
    pub custom_number: String,
    #[serde(default)]
    pub remark: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MotionPoint {
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub delay_ms: i32,
}

#[derive(Clone, Debug)]
pub enum BatchAction {
    KeyEvent(String),
    Text(String),
    Tap {
        x: i32,
        y: i32,
    },
    Swipe {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: i32,
    },
    GesturePath {
        points: Vec<MotionPoint>,
    },
    Launch {
        package: String,
        activity: String,
    },
    Stop {
        package: String,
    },
    Reboot,
    RebootRecovery,
    RebootBootloader,
    Uninstall {
        package: String,
    },
    ClearData {
        package: String,
    },
    Clipboard(String),
    Install {
        apk_path: PathBuf,
    },
    PushFile {
        local_path: PathBuf,
        remote_path: String,
    },
    PushFiles {
        local_paths: Vec<PathBuf>,
        remote_dir: String,
    },
    PullFile {
        remote_path: String,
        local_dir: Option<PathBuf>,
    },
    Shell {
        command: String,
    },
    Screenshot,
    Mirror,
}

impl fmt::Display for BatchAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchAction::KeyEvent(key) => write!(f, "按键 {key}"),
            BatchAction::Text(_) => write!(f, "输入文本"),
            BatchAction::Tap { .. } => write!(f, "点击"),
            BatchAction::Swipe { .. } => write!(f, "滑动"),
            BatchAction::GesturePath { .. } => write!(f, "轨迹滑动"),
            BatchAction::Launch { package, .. } => write!(f, "启动 {package}"),
            BatchAction::Stop { package } => write!(f, "停止 {package}"),
            BatchAction::Reboot => write!(f, "重启设备"),
            BatchAction::RebootRecovery => write!(f, "重启到 Recovery"),
            BatchAction::RebootBootloader => write!(f, "重启到 Bootloader"),
            BatchAction::Uninstall { package } => write!(f, "卸载 {package}"),
            BatchAction::ClearData { package } => write!(f, "清除 {package} 数据"),
            BatchAction::Clipboard(_) => write!(f, "发送到剪贴板"),
            BatchAction::Install { .. } => write!(f, "安装 APK"),
            BatchAction::PushFile { .. } => write!(f, "传文件到手机"),
            BatchAction::PushFiles { .. } => write!(f, "分发多个文件"),
            BatchAction::PullFile { .. } => write!(f, "导出手机文件"),
            BatchAction::Shell { .. } => write!(f, "ADB Shell"),
            BatchAction::Screenshot => write!(f, "截图"),
            BatchAction::Mirror => write!(f, "投屏"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ActionResult {
    pub serial: String,
    pub label: String,
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub artifact_path: Option<PathBuf>,
    pub duration_ms: u128,
}

#[derive(Clone, Debug, Default)]
pub struct EnvironmentStatus {
    pub adb_path: Option<PathBuf>,
    pub scrcpy_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct PreviewFrame {
    pub serial: String,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub error: String,
}

pub fn detect_environment() -> EnvironmentStatus {
    EnvironmentStatus {
        adb_path: find_binary("adb"),
        scrcpy_path: find_binary("scrcpy"),
    }
}

pub fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let output = run_text_command(
        Command::new("adb")
            .arg("devices")
            .arg("-l")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?;

    let mut devices = parse_devices(&output.stdout);
    let online_indices: Vec<usize> = devices
        .iter()
        .enumerate()
        .filter_map(|(idx, device)| (device.state == "device").then_some(idx))
        .collect();

    let mut handles = Vec::new();
    for index in online_indices {
        let serial = devices[index].serial.clone();
        handles.push(thread::spawn(move || (index, get_device_props(&serial))));
    }

    for handle in handles {
        if let Ok((index, props)) = handle.join() {
            if let Some(device) = devices.get_mut(index) {
                if let Some(value) = props.model.filter(|value| !value.is_empty()) {
                    device.model = value;
                }
                if let Some(value) = props.brand.filter(|value| !value.is_empty()) {
                    device.brand = value;
                }
                if let Some(value) = props.android_version.filter(|value| !value.is_empty()) {
                    device.android_version = value;
                }
            }
        }
    }

    devices.sort_by(|a, b| a.serial.cmp(&b.serial));
    Ok(devices)
}

pub fn adb_server_command(command: &str) -> Result<CommandOutput, String> {
    let normalized = command.trim();
    if normalized != "kill-server" && normalized != "start-server" {
        return Err("不支持的 ADB 服务命令".to_string());
    }
    run_text_command(Command::new("adb").arg(normalized))
}

pub fn adb_tcpip(serial: &str, port: u16) -> Result<CommandOutput, String> {
    if serial.trim().is_empty() {
        return Err("缺少设备序列号".to_string());
    }
    run_text_command(
        Command::new("adb")
            .arg("-s")
            .arg(serial.trim())
            .arg("tcpip")
            .arg(port.to_string()),
    )
}

pub fn adb_connect(target: &str) -> Result<CommandOutput, String> {
    let normalized = target.trim();
    if normalized.is_empty() {
        return Err("缺少连接目标".to_string());
    }
    run_text_command(Command::new("adb").arg("connect").arg(normalized))
}

pub fn adb_disconnect(target: Option<&str>) -> Result<CommandOutput, String> {
    let mut command = Command::new("adb");
    command.arg("disconnect");
    if let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) {
        command.arg(target);
    }
    run_text_command(&mut command)
}

pub fn read_logcat(serial: &str, lines: usize, filter_spec: Option<&str>) -> Result<CommandOutput, String> {
    if serial.trim().is_empty() {
        return Err("缺少设备序列号".to_string());
    }
    let mut command = Command::new("adb");
    command
        .arg("-s")
        .arg(serial.trim())
        .arg("logcat")
        .arg("-d")
        .arg("-t")
        .arg(lines.max(20).min(2000).to_string());
    if let Some(filter) = filter_spec.map(str::trim).filter(|value| !value.is_empty()) {
        for token in filter.split_whitespace() {
            command.arg(token);
        }
    }
    run_text_command(&mut command)
}

pub fn clear_logcat(serial: &str) -> Result<CommandOutput, String> {
    if serial.trim().is_empty() {
        return Err("缺少设备序列号".to_string());
    }
    run_text_command(
        Command::new("adb")
            .arg("-s")
            .arg(serial.trim())
            .arg("logcat")
            .arg("-c"),
    )
}

pub fn run_batch(
    serials: Vec<String>,
    action: BatchAction,
    screenshot_dir: PathBuf,
) -> Vec<ActionResult> {
    run_batch_bounded(serials, action, screenshot_dir, 12)
}

fn run_batch_bounded(
    serials: Vec<String>,
    action: BatchAction,
    screenshot_dir: PathBuf,
    max_concurrent: usize,
) -> Vec<ActionResult> {
    let max_concurrent = max_concurrent.max(1).min(32);
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let mut pending = 0usize;
    let mut serial_iter = serials.into_iter();
    let mut results = Vec::new();

    // Seed initial batch
    for serial in serial_iter.by_ref().take(max_concurrent) {
        let tx = result_tx.clone();
        let action = action.clone();
        let dir = screenshot_dir.clone();
        thread::spawn(move || {
            let r = run_action(serial, action, dir);
            let _ = tx.send(r);
        });
        pending += 1;
    }

    // As each completes, launch next
    while pending > 0 {
        if let Ok(result) = result_rx.recv() {
            results.push(result);
            pending -= 1;
            if let Some(serial) = serial_iter.next() {
                let tx = result_tx.clone();
                let action = action.clone();
                let dir = screenshot_dir.clone();
                thread::spawn(move || {
                    let r = run_action(serial, action, dir);
                    let _ = tx.send(r);
                });
                pending += 1;
            }
        }
    }

    results.sort_by(|a, b| a.serial.cmp(&b.serial));
    results
}

pub fn capture_previews(serials: Vec<String>, target_height: u32) -> Vec<PreviewFrame> {
    let max_concurrent: usize = 8;
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let mut pending = 0usize;
    let mut serial_iter = serials.into_iter();
    let mut frames = Vec::new();

    // Seed initial batch
    for serial in serial_iter.by_ref().take(max_concurrent) {
        let tx = result_tx.clone();
        thread::spawn(move || {
            let f = capture_preview_frame(&serial, target_height);
            let _ = tx.send(f);
        });
        pending += 1;
    }

    // As each completes, launch next
    while pending > 0 {
        if let Ok(frame) = result_rx.recv() {
            frames.push(frame);
            pending -= 1;
            if let Some(serial) = serial_iter.next() {
                let tx = result_tx.clone();
                thread::spawn(move || {
                    let f = capture_preview_frame(&serial, target_height);
                    let _ = tx.send(f);
                });
                pending += 1;
            }
        }
    }

    frames.sort_by(|a, b| a.serial.cmp(&b.serial));
    frames
}

pub fn capture_preview_frame(serial: &str, target_height: u32) -> PreviewFrame {
    capture_preview(serial, target_height)
}

pub fn capture_preview_png(serial: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("adb")
        .arg("-s")
        .arg(serial)
        .arg("exec-out")
        .arg("screencap")
        .arg("-p")
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(output.stdout)
}

pub fn capture_preview_jpeg(serial: &str, target_height: u32, quality: u8) -> Result<Vec<u8>, String> {
    let png = capture_preview_png(serial)?;
    let image = image::load_from_memory(&png).map_err(|err| err.to_string())?;
    let resized = image.thumbnail(1024, target_height.max(200));
    let rgb = resized.to_rgb8();
    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut encoded, quality.clamp(25, 90));
    encoder
        .encode_image(&rgb)
        .map_err(|err| err.to_string())?;
    Ok(encoded)
}

pub fn list_remote_files(serial: &str, path: &str) -> Result<Vec<RemoteFileEntry>, String> {
    let target = normalize_remote_dir(path);
    let output = run_text_command(
        Command::new("adb")
            .arg("-s")
            .arg(serial)
            .arg("shell")
            .arg("ls")
            .arg("-ap1")
            .arg(&target),
    )?;

    let mut entries = Vec::new();
    if target != "/" {
        let parent = parent_remote_dir(&target);
        entries.push(RemoteFileEntry {
            name: "..".to_string(),
            path: parent,
            is_dir: true,
        });
    }

    for line in output.stdout.lines() {
        let name = line.trim();
        if name.is_empty() || name == "." || name == ".." || name == "./" || name == "../" {
            continue;
        }
        let is_dir = name.ends_with('/');
        let display_name = name.trim_end_matches('/').to_string();
        let full_path = join_remote_path(&target, &display_name);
        entries.push(RemoteFileEntry {
            name: display_name,
            path: full_path,
            is_dir,
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

/// Escape a string for safe interpolation into an ADB shell command.
/// Wraps in single quotes, escaping any embedded single quotes.
fn shell_escape(input: &str) -> String {
    let escaped = input.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Validate that a string looks like a legitimate Android package name.
fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn run_action(serial: String, action: BatchAction, screenshot_dir: PathBuf) -> ActionResult {
    let started_at = Instant::now();
    let label = action.to_string();
    let mut result = ActionResult {
        serial: serial.clone(),
        label,
        ok: false,
        ..Default::default()
    };

    let command_result = match action {
        BatchAction::KeyEvent(key) => {
            let cmd = format!("input keyevent {}", normalize_keyevent(&key));
            fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                status_ok: true,
                stdout: out,
                stderr: String::new(),
                artifact_path: None,
            })
        }
        BatchAction::Text(text) => {
            // 优先剪贴板+粘贴（~200ms），失败则回退 input text（慢）
            fast_text_via_clipboard(&serial, &text).or_else(|_| {
                let encoded = text.replace(' ', "%s");
                let cmd = format!("input text {}", shell_escape(&encoded));
                fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                    status_ok: true,
                    stdout: out,
                    stderr: String::new(),
                    artifact_path: None,
                })
            })
        }
        BatchAction::Tap { x, y } => {
            let cmd = format!("input tap {x} {y}");
            fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                status_ok: true,
                stdout: out,
                stderr: String::new(),
                artifact_path: None,
            })
        }
        BatchAction::Swipe {
            x1,
            y1,
            x2,
            y2,
            duration_ms,
        } => {
            let cmd = format!("input swipe {x1} {y1} {x2} {y2} {duration_ms}");
            fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                status_ok: true,
                stdout: out,
                stderr: String::new(),
                artifact_path: None,
            })
        }
        BatchAction::GesturePath { points } => run_gesture_path(&serial, &points),
        BatchAction::Launch { package, activity } => {
            if package.trim().is_empty() {
                Err("包名不能为空".to_string())
            } else if !is_valid_package_name(package.trim()) {
                Err("包名包含非法字符".to_string())
            } else if activity.trim().is_empty() {
                let cmd = format!(
                    "monkey -p {} -c android.intent.category.LAUNCHER 1",
                    shell_escape(package.trim())
                );
                fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                    status_ok: true,
                    stdout: out,
                    stderr: String::new(),
                    artifact_path: None,
                })
            } else {
                let target = if activity.contains('/') {
                    activity
                } else {
                    format!("{}/{}", package.trim(), activity.trim())
                };
                let cmd = format!("am start -n {}", shell_escape(&target));
                fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                    status_ok: true,
                    stdout: out,
                    stderr: String::new(),
                    artifact_path: None,
                })
            }
        }
        BatchAction::Stop { package } => {
            if package.trim().is_empty() {
                Err("包名不能为空".to_string())
            } else if !is_valid_package_name(package.trim()) {
                Err("包名包含非法字符".to_string())
            } else {
                let cmd = format!("am force-stop {}", shell_escape(package.trim()));
                fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                    status_ok: true,
                    stdout: out,
                    stderr: String::new(),
                    artifact_path: None,
                })
            }
        }
        BatchAction::Reboot => adb_command(&serial, &["reboot"]),
        BatchAction::RebootRecovery => adb_command(&serial, &["reboot", "recovery"]),
        BatchAction::RebootBootloader => adb_command(&serial, &["reboot", "bootloader"]),
        BatchAction::Uninstall { package } => {
            if package.trim().is_empty() {
                Err("包名不能为空".to_string())
            } else if !is_valid_package_name(package.trim()) {
                Err("包名包含非法字符".to_string())
            } else {
                adb_command(&serial, &["uninstall", package.trim()])
            }
        }
        BatchAction::ClearData { package } => {
            if package.trim().is_empty() {
                Err("包名不能为空".to_string())
            } else if !is_valid_package_name(package.trim()) {
                Err("包名包含非法字符".to_string())
            } else {
                let cmd = format!("pm clear {}", shell_escape(package.trim()));
                fast_shell_command(&serial, &cmd).map(|out| CommandOutput {
                    status_ok: true,
                    stdout: out,
                    stderr: String::new(),
                    artifact_path: None,
                })
            }
        }
        BatchAction::Clipboard(text) => set_clipboard_text(&serial, &text),
        BatchAction::Install { apk_path } => {
            if !apk_path.exists() {
                Err(format!("APK 不存在: {}", apk_path.display()))
            } else {
                run_text_command(
                    Command::new("adb")
                        .arg("-s")
                        .arg(&serial)
                        .arg("install")
                        .arg("-r")
                        .arg(apk_path),
                )
            }
        }
        BatchAction::PushFile {
            local_path,
            remote_path,
        } => push_file_to_device(&serial, &local_path, &remote_path),
        BatchAction::PushFiles {
            local_paths,
            remote_dir,
        } => push_files_to_device(&serial, &local_paths, &remote_dir),
        BatchAction::PullFile {
            remote_path,
            local_dir,
        } => pull_file_from_device(&serial, &remote_path, local_dir.as_deref(), &screenshot_dir),
        // Shell is intentionally raw — the user explicitly wants arbitrary shell execution,
        // so no escaping is applied here. This is by design.
        BatchAction::Shell { command } => {
            if command.trim().is_empty() {
                Err("Shell 命令不能为空".to_string())
            } else {
                fast_shell_command(&serial, command.trim()).map(|out| CommandOutput {
                    status_ok: true,
                    stdout: out,
                    stderr: String::new(),
                    artifact_path: None,
                })
            }
        }
        BatchAction::Screenshot => {
            capture_screenshot(&serial, &screenshot_dir).map(|path| CommandOutput {
                status_ok: true,
                stdout: "截图成功".to_string(),
                stderr: String::new(),
                artifact_path: Some(path),
            })
        }
        BatchAction::Mirror => open_mirror(&serial).map(|_| CommandOutput {
            status_ok: true,
            stdout: "已尝试打开 scrcpy".to_string(),
            stderr: String::new(),
            artifact_path: None,
        }),
    };

    match command_result {
        Ok(output) => {
            result.ok = output.status_ok;
            result.stdout = output.stdout;
            result.stderr = output.stderr;
            result.artifact_path = output.artifact_path;
        }
        Err(message) => {
            result.stderr = message;
        }
    }

    result.duration_ms = started_at.elapsed().as_millis();
    result
}

fn adb_command(serial: &str, args: &[&str]) -> Result<CommandOutput, String> {
    let mut command = Command::new("adb");
    command.arg("-s").arg(serial);
    for arg in args {
        command.arg(arg);
    }
    run_text_command(&mut command)
}

fn run_gesture_path(serial: &str, points: &[MotionPoint]) -> Result<CommandOutput, String> {
    let points = sanitize_motion_points(points);
    if points.is_empty() {
        return Err("轨迹不能为空".to_string());
    }

    if points.len() == 1 {
        let point = &points[0];
        let cmd = format!("input tap {} {}", point.x, point.y);
        return fast_shell_command(serial, &cmd).map(|out| CommandOutput {
            status_ok: true,
            stdout: out,
            stderr: String::new(),
            artifact_path: None,
        });
    }

    let first = &points[0];
    let last = points.last().unwrap_or(first);
    let mut script = Vec::with_capacity(points.len() * 2 + 2);
    script.push(format!("input motionevent DOWN {} {}", first.x, first.y));

    for point in points.iter().skip(1) {
        let delay_micros = point.delay_ms.clamp(8, 60) * 1000;
        script.push(format!("usleep {}", delay_micros));
        script.push(format!("input motionevent MOVE {} {}", point.x, point.y));
    }

    script.push("usleep 16000".to_string());
    script.push(format!("input motionevent UP {} {}", last.x, last.y));

    fast_shell_command(serial, &script.join("; ")).map(|out| CommandOutput {
        status_ok: true,
        stdout: out,
        stderr: String::new(),
        artifact_path: None,
    })
}

fn sanitize_motion_points(points: &[MotionPoint]) -> Vec<MotionPoint> {
    let mut sanitized = Vec::new();

    for point in points.iter().take(64) {
        let next = MotionPoint {
            x: point.x.max(0),
            y: point.y.max(0),
            delay_ms: point.delay_ms.clamp(0, 120),
        };

        if sanitized
            .last()
            .map(|current: &MotionPoint| current.x == next.x && current.y == next.y)
            .unwrap_or(false)
        {
            continue;
        }

        sanitized.push(next);
    }

    if let Some(first) = sanitized.first_mut() {
        first.delay_ms = 0;
    }

    sanitized
}

fn capture_screenshot(serial: &str, screenshot_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(screenshot_dir).map_err(|err| err.to_string())?;
    let file_name = format!("{}-{}.png", serial.replace(':', "_"), timestamp_seconds());
    let file_path = screenshot_dir.join(file_name);

    let output = Command::new("adb")
        .arg("-s")
        .arg(serial)
        .arg("exec-out")
        .arg("screencap")
        .arg("-p")
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    std::fs::write(&file_path, output.stdout).map_err(|err| err.to_string())?;
    Ok(file_path)
}

fn push_file_to_device(
    serial: &str,
    local_path: &Path,
    remote_path: &str,
) -> Result<CommandOutput, String> {
    if !local_path.exists() {
        return Err(format!("文件不存在: {}", local_path.display()));
    }
    let remote_path = remote_path.trim();
    if remote_path.is_empty() {
        return Err("手机目标路径不能为空".to_string());
    }

    run_text_command(
        Command::new("adb")
            .arg("-s")
            .arg(serial)
            .arg("push")
            .arg(local_path)
            .arg(remote_path),
    )
}

fn push_files_to_device(
    serial: &str,
    local_paths: &[PathBuf],
    remote_dir: &str,
) -> Result<CommandOutput, String> {
    if local_paths.is_empty() {
        return Err("至少选择一个本地文件".to_string());
    }
    let remote_dir = remote_dir.trim();
    if remote_dir.is_empty() {
        return Err("手机目标目录不能为空".to_string());
    }

    let mut stdout = Vec::new();
    for local_path in local_paths {
        if !local_path.exists() {
            return Err(format!("文件不存在: {}", local_path.display()));
        }
        let output = run_text_command(
            Command::new("adb")
                .arg("-s")
                .arg(serial)
                .arg("push")
                .arg(local_path)
                .arg(remote_dir),
        )?;
        if !output.stdout.trim().is_empty() {
            stdout.push(output.stdout.trim().to_string());
        }
        if !output.stderr.trim().is_empty() {
            stdout.push(output.stderr.trim().to_string());
        }
    }

    Ok(CommandOutput {
        status_ok: true,
        stdout: stdout.join("\n"),
        stderr: String::new(),
        artifact_path: None,
    })
}

fn pull_file_from_device(
    serial: &str,
    remote_path: &str,
    local_dir: Option<&Path>,
    artifact_root: &Path,
) -> Result<CommandOutput, String> {
    let remote_path = remote_path.trim();
    if remote_path.is_empty() {
        return Err("手机文件路径不能为空".to_string());
    }

    let target_dir = match local_dir {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => default_download_dir(artifact_root),
    };
    std::fs::create_dir_all(&target_dir).map_err(|err| err.to_string())?;

    let file_name = remote_basename(remote_path);
    let target_path = target_dir.join(format!("{}-{}", serial.replace(':', "_"), file_name));
    let output = run_text_command(
        Command::new("adb")
            .arg("-s")
            .arg(serial)
            .arg("pull")
            .arg(remote_path)
            .arg(&target_path),
    )?;

    Ok(CommandOutput {
        artifact_path: Some(target_path),
        ..output
    })
}

fn default_download_dir(artifact_root: &Path) -> PathBuf {
    artifact_root
        .parent()
        .unwrap_or(artifact_root)
        .join("downloads")
}

fn remote_basename(remote_path: &str) -> String {
    let trimmed = remote_path.trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("device-file.bin")
        .to_string()
}

fn normalize_remote_dir(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/sdcard/Download".to_string()
    } else if trimmed == "/" {
        "/".to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

fn parent_remote_dir(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }
    let trimmed = path.trim_end_matches('/');
    let parent = trimmed
        .rsplit_once('/')
        .map(|(head, _)| head)
        .unwrap_or("/");
    if parent.is_empty() {
        "/".to_string()
    } else {
        parent.to_string()
    }
}

fn join_remote_path(base: &str, child: &str) -> String {
    if base == "/" {
        format!("/{child}")
    } else {
        format!("{}/{}", base.trim_end_matches('/'), child)
    }
}

fn set_clipboard_text(serial: &str, text: &str) -> Result<CommandOutput, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("剪贴板文本不能为空".to_string());
    }

    let attempts: [Vec<String>; 2] = [
        vec![
            "service".to_string(),
            "call".to_string(),
            "clipboard".to_string(),
            "2".to_string(),
            "i32".to_string(),
            "1".to_string(),
            "i32".to_string(),
            "0".to_string(),
            "s16".to_string(),
            "com.android.shell".to_string(),
            "s16".to_string(),
            trimmed.to_string(),
        ],
        vec![
            "service".to_string(),
            "call".to_string(),
            "clipboard".to_string(),
            "2".to_string(),
            "i32".to_string(),
            "1".to_string(),
            "s16".to_string(),
            "com.android.shell".to_string(),
            "s16".to_string(),
            trimmed.to_string(),
        ],
    ];

    let mut last_error = String::new();
    for args in attempts {
        let mut command = Command::new("adb");
        command.arg("-s").arg(serial).arg("shell");
        for arg in &args {
            command.arg(arg);
        }
        let output = run_text_command(&mut command)?;
        let combined = format!("{}\n{}", output.stdout, output.stderr).to_lowercase();
        if combined.contains("not allowed to perform write_clipboard")
            || combined.contains("permission denial")
            || combined.contains("securityexception")
            || output.stdout.contains("ffffffff")
            || output.stdout.contains("fffffffd")
        {
            last_error = "当前设备系统禁止 shell 写入系统剪贴板，MIUI / Android 10 常见。可以改用“发送文本”或后续接入辅助输入法方案。".to_string();
            continue;
        }
        if output.status_ok {
            return Ok(CommandOutput {
                status_ok: true,
                stdout: "已尝试写入系统剪贴板".to_string(),
                stderr: output.stderr,
                artifact_path: None,
            });
        }
    }

    Err(if last_error.is_empty() {
        "当前设备暂不支持直接写入系统剪贴板".to_string()
    } else {
        last_error
    })
}

/// 通过剪贴板+粘贴实现快速文本输入（~200ms vs input text 的 6s+）
pub fn fast_text_via_clipboard(serial: &str, text: &str) -> Result<CommandOutput, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("文本为空".to_string());
    }
    let escaped = shell_escape(trimmed);

    // Android 11+ (API 30): service call clipboard 2 i32 1 i32 0 s16 pkg s16 text
    let set_cmd = format!(
        "service call clipboard 2 i32 1 i32 0 s16 com.android.shell s16 {escaped}"
    );
    let set_result = fast_shell_command(serial, &set_cmd)?;

    let lower = set_result.to_lowercase();
    if lower.contains("ffffffff") || lower.contains("permission") || lower.contains("security") {
        // Android 9-10 fallback: without i32 0
        let set_cmd2 = format!(
            "service call clipboard 2 i32 1 s16 com.android.shell s16 {escaped}"
        );
        let set_result2 = fast_shell_command(serial, &set_cmd2)?;
        let lower2 = set_result2.to_lowercase();
        if lower2.contains("ffffffff") || lower2.contains("permission") || lower2.contains("security") {
            return Err("剪贴板写入被拒绝".to_string());
        }
    }

    // 粘贴: KEYCODE_PASTE = 279
    fast_shell_command(serial, "input keyevent 279")?;

    Ok(CommandOutput {
        status_ok: true,
        stdout: "文本已通过剪贴板粘贴".to_string(),
        stderr: String::new(),
        artifact_path: None,
    })
}

fn capture_preview(serial: &str, target_height: u32) -> PreviewFrame {
    let output = Command::new("adb")
        .arg("-s")
        .arg(serial)
        .arg("exec-out")
        .arg("screencap")
        .arg("-p")
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) => {
            return PreviewFrame {
                serial: serial.to_string(),
                error: err.to_string(),
                ..Default::default()
            };
        }
    };

    if !output.status.success() {
        return PreviewFrame {
            serial: serial.to_string(),
            error: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ..Default::default()
        };
    }

    let decoded = image::load_from_memory(&output.stdout);
    let image = match decoded {
        Ok(image) => image,
        Err(err) => {
            return PreviewFrame {
                serial: serial.to_string(),
                error: err.to_string(),
                ..Default::default()
            };
        }
    };

    let (width, height) = image.dimensions();
    let resized = image.thumbnail(1024, target_height.max(200));
    let rgba = resized.to_rgba8();

    PreviewFrame {
        serial: serial.to_string(),
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        rgba: rgba.into_raw(),
        error: if width == 0 || height == 0 {
            "空白截图".to_string()
        } else {
            String::new()
        },
    }
}

fn open_mirror(serial: &str) -> Result<(), String> {
    let _ = spawn_mirror_process(serial)?;
    Ok(())
}

pub fn spawn_mirror_process(serial: &str) -> Result<Child, String> {
    let scrcpy = find_binary("scrcpy").ok_or_else(|| "没有检测到 scrcpy".to_string())?;
    let mut command = Command::new(scrcpy);
    command
        .arg("-s")
        .arg(serial)
        .arg("--window-title")
        .arg(format!("群控实时画面 · {serial}"))
        .arg("--max-size")
        .arg("720")
        .arg("--video-codec")
        .arg("h264")
        .arg("--no-audio");

    configure_scrcpy_profile(serial, &mut command);

    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| err.to_string())
}

fn configure_scrcpy_profile(serial: &str, command: &mut Command) {
    if is_redroid_rk3588(serial) {
        command
            .arg("--video-encoder")
            .arg("c2.rk.avc.encoder")
            .arg("--video-bit-rate")
            .arg("4M")
            .arg("--max-fps")
            .arg("30")
            .arg("--display-buffer=0")
            .arg("--no-key-repeat");
        return;
    }

    command
        .arg("--video-bit-rate")
        .arg("4M")
        .arg("--max-fps")
        .arg("30")
        .arg("--display-buffer=0")
        .arg("--no-key-repeat");
}

fn is_redroid_rk3588(serial: &str) -> bool {
    let board = adb_prop(serial, "ro.board.platform").unwrap_or_default();
    if board.eq_ignore_ascii_case("rk3588") {
        return true;
    }

    let brand = adb_prop(serial, "ro.product.brand").unwrap_or_default();
    let model = adb_prop(serial, "ro.product.model").unwrap_or_default();
    brand.eq_ignore_ascii_case("redroid") || model.to_ascii_lowercase().contains("redroid")
}

fn parse_devices(output: &str) -> Vec<DeviceInfo> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = parts.next()?.to_string();
            let mut device = DeviceInfo {
                serial,
                state,
                ..Default::default()
            };

            for token in parts {
                if let Some((key, value)) = token.split_once(':') {
                    match key {
                        "product" => device.product = value.to_string(),
                        "model" => {
                            device.model_hint = value.to_string();
                            device.model = value.replace('_', " ");
                        }
                        "device" => device.device_hint = value.to_string(),
                        "transport_id" => device.transport_id = value.to_string(),
                        _ => {}
                    }
                }
            }

            Some(device)
        })
        .collect()
}

fn get_device_props(serial: &str) -> DeviceProps {
    DeviceProps {
        model: adb_prop(serial, "ro.product.model"),
        brand: adb_prop(serial, "ro.product.brand"),
        android_version: adb_prop(serial, "ro.build.version.release"),
    }
}

fn adb_prop(serial: &str, prop: &str) -> Option<String> {
    run_text_command(
        Command::new("adb")
            .arg("-s")
            .arg(serial)
            .arg("shell")
            .arg("getprop")
            .arg(prop),
    )
    .ok()
    .map(|output| output.stdout.trim().to_string())
    .filter(|value| !value.is_empty())
}

fn normalize_keyevent(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.chars().all(|char| char.is_ascii_digit()) {
        trimmed.to_string()
    } else if trimmed.starts_with("KEYCODE_") {
        trimmed.to_string()
    } else {
        format!("KEYCODE_{}", trimmed.to_uppercase())
    }
}

fn run_text_command(command: &mut Command) -> Result<CommandOutput, String> {
    let output = command.output().map_err(|err| err.to_string())?;
    Ok(CommandOutput {
        status_ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        artifact_path: None,
    })
}

fn find_binary(binary: &str) -> Option<PathBuf> {
    if binary.contains('/') {
        let path = PathBuf::from(binary);
        return path.exists().then_some(path);
    }

    let paths = std::env::var_os("PATH")?;
    for path in std::env::split_paths(&paths) {
        let candidate = path.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

#[derive(Debug, Default)]
struct DeviceProps {
    model: Option<String>,
    brand: Option<String>,
    android_version: Option<String>,
}

#[derive(Debug, Default)]
pub struct CommandOutput {
    pub status_ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub artifact_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Persistent ADB Shell connection pool
// ---------------------------------------------------------------------------

const SHELL_END_MARKER: &str = "___END_MARKER_";
static MARKER_COUNTER: AtomicU64 = AtomicU64::new(0);

static SHELL_POOL: LazyLock<Mutex<HashMap<String, PersistentShell>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct PersistentShell {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl PersistentShell {
    fn spawn(serial: &str) -> Result<Self, String> {
        let mut child = Command::new("adb")
            .arg("-s")
            .arg(serial)
            .arg("shell")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("启动持久 shell 失败: {e}"))?;

        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;

        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
        })
    }

    fn execute(&mut self, cmd: &str) -> Result<String, String> {
        let nonce = MARKER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let marker = format!("{SHELL_END_MARKER}{nonce}___");
        let full = format!("{cmd}; echo {marker}\n");
        self.stdin
            .write_all(full.as_bytes())
            .map_err(|e| format!("写入失败: {e}"))?;
        self.stdin.flush().map_err(|e| format!("flush 失败: {e}"))?;

        let mut output = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => return Err("shell 连接断开".to_string()),
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed == marker {
                        break;
                    }
                    output.push_str(&line);
                }
                Err(e) => return Err(format!("读取失败: {e}")),
            }
        }

        Ok(output.trim_end().to_string())
    }

    fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}

impl Drop for PersistentShell {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Execute a command via persistent shell (low-latency, no fork overhead).
/// Used for high-frequency operations like tap, swipe, text, keyevent.
pub fn fast_shell_command(serial: &str, cmd: &str) -> Result<String, String> {
    let mut shell = {
        let mut pool = SHELL_POOL.lock().map_err(|e| format!("锁定失败: {e}"))?;
        pool.remove(serial)
    };

    let needs_new = match &mut shell {
        Some(s) => !s.is_alive(),
        None => true,
    };

    if needs_new {
        drop(shell);
        shell = Some(PersistentShell::spawn(serial)?);
    }

    let mut shell = shell.unwrap();
    let result = shell.execute(cmd);

    if shell.is_alive() {
        if let Ok(mut pool) = SHELL_POOL.lock() {
            pool.insert(serial.to_string(), shell);
        }
    }

    result
}

/// Drop all persistent shells.
pub fn close_all_shells() {
    if let Ok(mut pool) = SHELL_POOL.lock() {
        pool.clear();
    }
}

/// 预热 Shell 连接池：为指定设备提前创建持久 shell，消除首次操作的 ~2.5s 冷启动
pub fn warm_shell_pool(serials: &[String]) {
    let to_warm: Vec<String> = {
        let pool = match SHELL_POOL.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        serials
            .iter()
            .filter(|s| !pool.contains_key(s.as_str()))
            .cloned()
            .collect()
    };

    if to_warm.is_empty() {
        return;
    }

    let handles: Vec<_> = to_warm
        .into_iter()
        .map(|serial| {
            thread::spawn(move || {
                if let Ok(shell) = PersistentShell::spawn(&serial) {
                    if let Ok(mut pool) = SHELL_POOL.lock() {
                        pool.insert(serial, shell);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        let _ = handle.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // parse_devices
    // -----------------------------------------------------------------------

    #[test]
    fn parse_devices_basic() {
        let output = "\
List of devices attached
emulator-5554          device product:sdk_gphone64_arm64 model:sdk_gphone64_arm64 device:emu64a transport_id:1
192.168.1.100:5555     device product:full model:Redmi_Note_8 device:ginkgo transport_id:5
ABCDEF123456           offline transport_id:3
";
        let devices = parse_devices(output);
        assert_eq!(devices.len(), 3);

        // First device — emulator
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, "device");
        assert_eq!(devices[0].product, "sdk_gphone64_arm64");
        assert_eq!(devices[0].model_hint, "sdk_gphone64_arm64");
        assert_eq!(devices[0].model, "sdk gphone64 arm64");
        assert_eq!(devices[0].device_hint, "emu64a");
        assert_eq!(devices[0].transport_id, "1");

        // Second device — network connected
        assert_eq!(devices[1].serial, "192.168.1.100:5555");
        assert_eq!(devices[1].state, "device");
        assert_eq!(devices[1].model_hint, "Redmi_Note_8");
        assert_eq!(devices[1].model, "Redmi Note 8");
        assert_eq!(devices[1].device_hint, "ginkgo");
        assert_eq!(devices[1].transport_id, "5");

        // Third device — offline
        assert_eq!(devices[2].serial, "ABCDEF123456");
        assert_eq!(devices[2].state, "offline");
        assert_eq!(devices[2].transport_id, "3");
        assert_eq!(devices[2].model, "");
    }

    #[test]
    fn parse_devices_empty() {
        let output = "List of devices attached\n\n";
        let devices = parse_devices(output);
        assert!(devices.is_empty());
    }

    // -----------------------------------------------------------------------
    // normalize_keyevent
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_keyevent_home() {
        assert_eq!(normalize_keyevent("HOME"), "KEYCODE_HOME");
    }

    #[test]
    fn normalize_keyevent_already_prefixed() {
        assert_eq!(normalize_keyevent("KEYCODE_BACK"), "KEYCODE_BACK");
    }

    #[test]
    fn normalize_keyevent_numeric() {
        assert_eq!(normalize_keyevent("66"), "66");
    }

    #[test]
    fn normalize_keyevent_trimmed_and_uppercased() {
        assert_eq!(normalize_keyevent("  enter  "), "KEYCODE_ENTER");
    }

    // -----------------------------------------------------------------------
    // sanitize_motion_points
    // -----------------------------------------------------------------------

    #[test]
    fn sanitize_motion_points_empty() {
        let result = sanitize_motion_points(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn sanitize_motion_points_negative_coords_clamped() {
        let points = vec![MotionPoint { x: -10, y: -20, delay_ms: 5 }];
        let result = sanitize_motion_points(&points);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].x, 0);
        assert_eq!(result[0].y, 0);
        // First point delay is always forced to 0
        assert_eq!(result[0].delay_ms, 0);
    }

    #[test]
    fn sanitize_motion_points_dedup_consecutive() {
        let points = vec![
            MotionPoint { x: 10, y: 20, delay_ms: 0 },
            MotionPoint { x: 10, y: 20, delay_ms: 5 },
            MotionPoint { x: 30, y: 40, delay_ms: 10 },
        ];
        let result = sanitize_motion_points(&points);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].x, 10);
        assert_eq!(result[1].x, 30);
    }

    #[test]
    fn sanitize_motion_points_truncate_to_64() {
        let points: Vec<MotionPoint> = (0..100)
            .map(|i| MotionPoint { x: i, y: i, delay_ms: 10 })
            .collect();
        let result = sanitize_motion_points(&points);
        assert!(result.len() <= 64);
    }

    #[test]
    fn sanitize_motion_points_first_delay_zero() {
        let points = vec![
            MotionPoint { x: 5, y: 5, delay_ms: 50 },
            MotionPoint { x: 10, y: 10, delay_ms: 30 },
        ];
        let result = sanitize_motion_points(&points);
        assert_eq!(result[0].delay_ms, 0);
    }

    // -----------------------------------------------------------------------
    // shell_escape
    // -----------------------------------------------------------------------

    #[test]
    fn shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn shell_escape_single_quote() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn shell_escape_semicolons() {
        assert_eq!(shell_escape("a; rm -rf /"), "'a; rm -rf /'");
    }

    #[test]
    fn shell_escape_empty() {
        assert_eq!(shell_escape(""), "''");
    }

    // -----------------------------------------------------------------------
    // is_valid_package_name
    // -----------------------------------------------------------------------

    #[test]
    fn is_valid_package_name_normal() {
        assert!(is_valid_package_name("com.example.app"));
    }

    #[test]
    fn is_valid_package_name_empty() {
        assert!(!is_valid_package_name(""));
    }

    #[test]
    fn is_valid_package_name_injection() {
        assert!(!is_valid_package_name("com.example; rm -rf /"));
    }

    #[test]
    fn is_valid_package_name_too_long() {
        let long = "a".repeat(257);
        assert!(!is_valid_package_name(&long));
    }

    // -----------------------------------------------------------------------
    // remote_basename
    // -----------------------------------------------------------------------

    #[test]
    fn remote_basename_file() {
        assert_eq!(remote_basename("/sdcard/Download/file.apk"), "file.apk");
    }

    #[test]
    fn remote_basename_trailing_slash() {
        assert_eq!(remote_basename("/sdcard/Download/"), "Download");
    }

    #[test]
    fn remote_basename_root_fallback() {
        assert_eq!(remote_basename("/"), "device-file.bin");
    }

    // -----------------------------------------------------------------------
    // normalize_remote_dir
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_remote_dir_empty() {
        assert_eq!(normalize_remote_dir(""), "/sdcard/Download");
    }

    #[test]
    fn normalize_remote_dir_root() {
        assert_eq!(normalize_remote_dir("/"), "/");
    }

    #[test]
    fn normalize_remote_dir_trailing_slash() {
        assert_eq!(normalize_remote_dir("/sdcard/Download/"), "/sdcard/Download");
    }

    // -----------------------------------------------------------------------
    // parent_remote_dir
    // -----------------------------------------------------------------------

    #[test]
    fn parent_remote_dir_root() {
        assert_eq!(parent_remote_dir("/"), "/");
    }

    #[test]
    fn parent_remote_dir_single_level() {
        assert_eq!(parent_remote_dir("/sdcard"), "/");
    }

    #[test]
    fn parent_remote_dir_two_levels() {
        assert_eq!(parent_remote_dir("/sdcard/Download"), "/sdcard");
    }

    // -----------------------------------------------------------------------
    // join_remote_path
    // -----------------------------------------------------------------------

    #[test]
    fn join_remote_path_from_root() {
        assert_eq!(join_remote_path("/", "sdcard"), "/sdcard");
    }

    #[test]
    fn join_remote_path_normal() {
        assert_eq!(join_remote_path("/sdcard", "Download"), "/sdcard/Download");
    }

    #[test]
    fn join_remote_path_trailing_slash() {
        assert_eq!(join_remote_path("/sdcard/", "Download"), "/sdcard/Download");
    }
}
