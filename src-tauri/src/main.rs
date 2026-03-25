use std::path::PathBuf;
use std::fs::OpenOptions;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

struct BackendState(Mutex<Option<Child>>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(BackendState(Mutex::new(None)))
        .setup(|app| {
            launch_backend(app.handle())?;
            setup_native_menu(app.handle())?;
            apply_native_window_effects(app.handle());
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().0.as_str();
            match id {
                "app.refresh_devices"
                | "app.refresh_previews"
                | "app.select_all_online"
                | "app.clear_selection"
                | "app.open_records"
                | "app.focus_search"
                | "app.open_preferences" => {
                    let _ = app.emit("native-command", id);
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build tauri app")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
                stop_backend(app);
            }
        });
}

fn apply_native_window_effects(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app.get_webview_window("main") {
            let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None);
        }
    }
}

fn setup_native_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let about = AboutMetadata {
        name: Some("群控主板".into()),
        version: Some("0.1.0".into()),
        short_version: Some("0.1.0".into()),
        comments: Some("Android 群控桌面控制台".into()),
        ..Default::default()
    };

    let refresh_devices = MenuItemBuilder::with_id("app.refresh_devices", "刷新设备")
        .accelerator("CmdOrCtrl+R")
        .build(app)?;
    let refresh_previews = MenuItemBuilder::with_id("app.refresh_previews", "刷新画面")
        .accelerator("CmdOrCtrl+Shift+R")
        .build(app)?;
    let select_all_online = MenuItemBuilder::with_id("app.select_all_online", "全选在线设备")
        .accelerator("CmdOrCtrl+Shift+A")
        .build(app)?;
    let clear_selection = MenuItemBuilder::with_id("app.clear_selection", "清空当前选择")
        .accelerator("Escape")
        .build(app)?;
    let open_records = MenuItemBuilder::with_id("app.open_records", "打开记录中心")
        .accelerator("CmdOrCtrl+2")
        .build(app)?;
    let focus_search = MenuItemBuilder::with_id("app.focus_search", "聚焦搜索")
        .accelerator("CmdOrCtrl+F")
        .build(app)?;
    let open_preferences = MenuItemBuilder::with_id("app.open_preferences", "偏好设置…")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let app_menu = SubmenuBuilder::new(app, "群控主板")
        .about(Some(about))
        .separator()
        .item(&open_preferences)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "文件")
        .item(&refresh_devices)
        .item(&refresh_previews)
        .separator()
        .item(&open_records)
        .close_window()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "编辑")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let device_menu = SubmenuBuilder::new(app, "设备")
        .item(&select_all_online)
        .item(&clear_selection)
        .item(&focus_search)
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "窗口")
        .minimize()
        .maximize()
        .fullscreen()
        .separator()
        .close_window()
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&device_menu)
        .item(&window_menu)
        .build()?;

    app.set_menu(menu)?;
    Ok(())
}

fn launch_backend(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    if wait_for_backend("127.0.0.1:8876", Duration::from_millis(250)) {
        return Ok(());
    }
    let backend_path = backend_binary_path(app)?;
    let mut child = spawn_backend_process(&backend_path, false)?;
    if wait_for_backend("127.0.0.1:8876", Duration::from_secs(5)) {
        let state = app.state::<BackendState>();
        let mut guard = state.0.lock().expect("backend mutex poisoned");
        *guard = Some(child);
        return Ok(());
    }

    let _ = child.kill();
    let _ = child.wait(); // reap the process
    std::thread::sleep(Duration::from_millis(500)); // wait for port release

    child = spawn_backend_process(&backend_path, true)?;
    if wait_for_backend("127.0.0.1:8876", Duration::from_secs(5)) {
        let state = app.state::<BackendState>();
        let mut guard = state.0.lock().expect("backend mutex poisoned");
        *guard = Some(child);
        return Ok(());
    }

    let _ = child.kill();
    Err("failed to start bundled backend".into())
}

fn merged_shell_path() -> String {
    let mut segments = Vec::new();
    if let Some(current) = std::env::var_os("PATH") {
        segments.extend(std::env::split_paths(&current));
    }
    for extra in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        let path = PathBuf::from(extra);
        if !segments.iter().any(|item| item == &path) {
            segments.push(path);
        }
    }
    std::env::join_paths(segments)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string())
}

fn backend_binary_path(app: &AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug/web"));
    }

    let executable_dir = std::env::current_exe()?
        .parent()
        .map(PathBuf::from)
        .ok_or("missing executable parent directory")?;
    let bundled_web = executable_dir.join("web");
    if bundled_web.exists() {
        return Ok(bundled_web);
    }

    let sidecar_name = format!("web-{}", env!("APP_TARGET_TRIPLE"));
    if let Ok(path) = app.path().resolve(&sidecar_name, BaseDirectory::Resource) {
        if path.exists() {
            return Ok(path);
        }
    }

    let fallback = executable_dir.join(&sidecar_name);
    Ok(fallback)
}

fn wait_for_backend(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(180));
    }
    false
}

fn stop_backend(app: &tauri::AppHandle) {
    let state = app.state::<BackendState>();
    if let Ok(mut guard) = state.0.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
}

fn spawn_backend_process(
    backend_path: &PathBuf,
    via_shell: bool,
) -> Result<Child, Box<dyn std::error::Error>> {
    let path = merged_shell_path();
    let base_dir = project_base_dir();
    let log_path = std::env::temp_dir().join("group-control-backend.log");
    // Rotate log if > 5MB
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() > 5 * 1024 * 1024 {
            let _ = std::fs::write(&log_path, ""); // truncate
        }
    }
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let mut command = if via_shell {
        let mut shell = Command::new("/bin/zsh");
        let escaped_backend = shell_escape_path(backend_path);
        let escaped_base_dir = shell_escape_path(&base_dir);
        shell.arg("-lc").arg(format!(
            "GROUP_CONTROL_HOST=127.0.0.1 GROUP_CONTROL_PORT=8876 GROUP_CONTROL_BASE_DIR={escaped_base_dir} PATH={} {escaped_backend}",
            shell_escape_value(&path)
        ));
        shell
    } else {
        let mut direct = Command::new(backend_path);
        direct
            .env("GROUP_CONTROL_HOST", "127.0.0.1")
            .env("GROUP_CONTROL_PORT", "8876")
            .env("GROUP_CONTROL_BASE_DIR", &base_dir)
            .env("PATH", &path)
            .current_dir(&base_dir);
        direct
    };

    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    Ok(command.spawn()?)
}

fn project_base_dir() -> PathBuf {
    // In debug mode, use compile-time path (CARGO_MANIFEST_DIR points to src-tauri/)
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    }
    // In release mode, derive from executable location
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            // macOS .app bundle: exe is in .app/Contents/MacOS/
            // base_dir should be the Resources dir or alongside the .app
            p.parent().map(|d| d.to_path_buf())
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn shell_escape_path(path: &PathBuf) -> String {
    shell_escape_value(&path.display().to_string())
}

fn shell_escape_value(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
