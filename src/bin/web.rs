use std::convert::Infallible;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path as StdPath;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_stream::stream;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::CorsLayer;

use futures::{StreamExt, SinkExt};

use android_group_control_rs::adb::{self, BatchAction, MotionPoint};
use android_group_control_rs::auto_connector::AutoConnector;
use android_group_control_rs::bezier;
use android_group_control_rs::host::{self, HostConfig};
use android_group_control_rs::host_monitor::{HostMonitor, HostStats};
use android_group_control_rs::preview::{PreviewHub, PreviewPriority};
use android_group_control_rs::scrcpy::{ScrcpyManager, GroupController, ControlEvent};
use android_group_control_rs::storage::{
    self, AccountBinding, AdbCommandPreset, ApkMetadataCacheEntry, DeviceGroup, DeviceProfile,
    MaterialAsset, QueueItem, QuickPhrase, RecordingStep, RecordingTemplate, TaskRunRecord,
    TaskTemplate, TextTemplatePreset,
};

struct AppState {
    base_dir: PathBuf,
    focus_mirror: Mutex<FocusMirrorState>,
    preview_hub: PreviewHub,
    auto_connector: Arc<AutoConnector>,
    host_monitor: Arc<HostMonitor>,
    scrcpy: ScrcpyManager,
}

#[derive(Default)]
struct FocusMirrorState {
    serial: Option<String>,
    child: Option<Child>,
}

#[derive(Serialize)]
struct StatusPayload {
    adb_available: bool,
    adb_path: Option<String>,
    scrcpy_available: bool,
    scrcpy_path: Option<String>,
    device_count: usize,
    focus_mirror_serial: Option<String>,
    focus_mirror_running: bool,
}

#[derive(Serialize)]
struct DevicesPayload {
    devices: Vec<adb::DeviceInfo>,
}

#[derive(Serialize)]
struct DeviceFilesPayload {
    serial: String,
    path: String,
    entries: Vec<adb::RemoteFileEntry>,
}

#[derive(Serialize)]
struct GroupsPayload {
    groups: Vec<DeviceGroup>,
}

#[derive(Serialize)]
struct DeviceProfilesPayload {
    profiles: Vec<DeviceProfile>,
}

#[derive(Serialize)]
struct TasksPayload {
    templates: Vec<TaskTemplate>,
    runs: Vec<TaskRunRecord>,
}

#[derive(Serialize)]
struct RecordingsPayload {
    recordings: Vec<RecordingTemplate>,
}

#[derive(Serialize)]
struct QuickPhrasesPayload {
    phrases: Vec<QuickPhrase>,
}

#[derive(Serialize)]
struct TextTemplatesPayload {
    templates: Vec<TextTemplatePreset>,
}

#[derive(Serialize)]
struct AdbPresetsPayload {
    presets: Vec<AdbCommandPreset>,
}

#[derive(Serialize)]
struct MaterialsPayload {
    materials: Vec<MaterialAsset>,
    directory: String,
}

#[derive(Serialize)]
struct AccountsPayload {
    accounts: Vec<AccountBinding>,
}

#[derive(Serialize)]
struct QueuePayload {
    items: Vec<QueueItem>,
}

#[derive(Deserialize)]
struct SaveGroupRequest {
    name: String,
    serials: Vec<String>,
}

#[derive(Deserialize)]
struct SaveDeviceProfileRequest {
    serial: String,
    #[serde(default)]
    custom_name: String,
    #[serde(default)]
    custom_number: String,
    #[serde(default)]
    remark: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct SaveQuickPhraseRequest {
    name: String,
    #[serde(default)]
    group: String,
    text: String,
}

#[derive(Deserialize)]
struct SaveTextTemplateRequest {
    name: String,
    #[serde(default)]
    group: String,
    template: String,
}

#[derive(Deserialize)]
struct SaveAdbPresetRequest {
    name: String,
    #[serde(default)]
    group: String,
    command: String,
}

#[derive(Deserialize)]
struct DeviceFilesQuery {
    serial: String,
    path: Option<String>,
}

#[derive(Deserialize)]
struct ActionRequest {
    serials: Vec<String>,
    action: String,
    #[serde(default)]
    params: Value,
    template_id: Option<String>,
    task_name: Option<String>,
}

#[derive(Deserialize)]
struct FastInputRequest {
    serial: String,
    action: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
struct TextDispatchEntry {
    serial: String,
    text: String,
}

#[derive(Deserialize)]
struct TextDispatchRequest {
    entries: Vec<TextDispatchEntry>,
    #[serde(default = "default_dispatch_mode")]
    mode: String,
    task_name: Option<String>,
}

#[derive(Deserialize)]
struct PreviewRefreshRequest {
    #[serde(default)]
    serials: Vec<String>,
    priority: Option<String>,
}

#[derive(Deserialize)]
struct AdbServerRequest {
    command: String,
}

#[derive(Deserialize)]
struct AdbTcpipRequest {
    serial: String,
    port: Option<u16>,
}

#[derive(Deserialize)]
struct AdbConnectRequest {
    target: String,
}

#[derive(Deserialize)]
struct AdbDisconnectRequest {
    target: Option<String>,
}

#[derive(Deserialize)]
struct LogcatRequest {
    serials: Vec<String>,
    lines: Option<usize>,
    filter: Option<String>,
}

#[derive(Serialize)]
struct CommandResultPayload {
    ok: bool,
    stdout: String,
    stderr: String,
}

#[derive(Serialize)]
struct LogcatEntryPayload {
    serial: String,
    ok: bool,
    stdout: String,
    stderr: String,
}

#[derive(Serialize)]
struct LogcatPayload {
    entries: Vec<LogcatEntryPayload>,
}

#[derive(Serialize)]
struct PreviewRefreshPayload {
    refreshed: usize,
    last_updated_ms: u64,
}

#[derive(Serialize)]
struct ScreenshotsPayload {
    screenshots: Vec<ScreenshotItem>,
    directory: String,
}

#[derive(Serialize)]
struct ScreenshotItem {
    file_name: String,
    serial: String,
    created_at: String,
    size_bytes: u64,
    image_url: String,
}

#[derive(Serialize)]
struct ScreenshotBundlePayload {
    file_name: String,
    artifact_path: String,
    download_url: String,
}

#[derive(Serialize)]
struct ApkUploadPayload {
    file_name: String,
    artifact_path: String,
    size_bytes: usize,
    package_name: Option<String>,
    version_name: Option<String>,
    version_code: Option<String>,
    inspect_error: Option<String>,
}

#[derive(Serialize)]
struct ApkUploadsPayload {
    apks: Vec<UploadedApkItem>,
    directory: String,
}

#[derive(Serialize)]
struct UploadedApkItem {
    file_name: String,
    artifact_path: String,
    size_bytes: u64,
    created_at: String,
    package_name: Option<String>,
    version_name: Option<String>,
    version_code: Option<String>,
    inspect_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ApkInspectInfo {
    package_name: Option<String>,
    version_name: Option<String>,
    version_code: Option<String>,
    inspect_error: Option<String>,
}

#[derive(Serialize)]
struct PreviewEventFrame {
    serial: String,
    jpeg_base64: String,
    updated_at_ms: u64,
}

#[derive(Serialize)]
struct PreviewEventPayload {
    frames: Vec<PreviewEventFrame>,
    updated_at_ms: u64,
}

fn default_dispatch_mode() -> String {
    "input".to_string()
}

#[derive(Serialize)]
struct ActionResponse {
    results: Vec<ActionResultPayload>,
}

#[derive(Serialize)]
struct ActionResultPayload {
    serial: String,
    label: String,
    ok: bool,
    stdout: String,
    stderr: String,
    artifact_path: Option<String>,
    duration_ms: u128,
}

#[derive(Deserialize)]
struct FocusMirrorRequest {
    serial: String,
}

#[derive(Serialize)]
struct FocusMirrorPayload {
    running: bool,
    serial: Option<String>,
}

#[derive(Deserialize)]
struct WhepBridgeRequest {
    serial: String,
}

#[derive(Deserialize)]
struct WhepBridgeQuery {
    serial: String,
}

#[derive(Serialize)]
struct WhepBridgePayload {
    serial: String,
    path_key: String,
    tmux_running: bool,
    supervisor_running: bool,
    ffmpeg_running: bool,
    scrcpy_running: bool,
    whep_url: String,
    log_path: String,
}

#[derive(Deserialize)]
struct SaveTaskTemplateRequest {
    name: String,
    action: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
struct RunTaskTemplateRequest {
    serials: Vec<String>,
}

#[derive(Deserialize)]
struct SaveRecordingRequest {
    name: String,
    #[serde(default)]
    steps: Vec<RecordingStep>,
}

#[derive(Deserialize)]
struct RunRecordingRequest {
    serials: Vec<String>,
}

#[derive(Deserialize)]
struct RunDraftRecordingRequest {
    serials: Vec<String>,
    #[serde(default)]
    steps: Vec<RecordingStep>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct SaveAccountRequest {
    #[serde(default)]
    platform: String,
    account_name: String,
    #[serde(default)]
    account_group: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    serial: String,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct SaveQueueItemRequest {
    name: String,
    kind: String,
    ref_id: String,
    #[serde(default)]
    serials: Vec<String>,
    #[serde(default)]
    note: String,
}

#[tokio::main]
async fn main() {
    let base_dir = std::env::var_os("GROUP_CONTROL_BASE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        });
    let _ = storage::ensure_layout(&base_dir);
    let host = std::env::var("GROUP_CONTROL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("GROUP_CONTROL_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8876);

    let auto_connector = AutoConnector::new(base_dir.clone());
    let host_monitor = HostMonitor::new(base_dir.clone());
    let state = Arc::new(AppState {
        base_dir,
        focus_mirror: Mutex::new(FocusMirrorState::default()),
        preview_hub: PreviewHub::new(),
        auto_connector,
        host_monitor,
        scrcpy: ScrcpyManager::new(),
    });
    let app = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/devices", get(get_devices))
        .route(
            "/api/device-profiles",
            get(get_device_profiles).post(save_device_profile),
        )
        .route(
            "/api/device-profiles/{serial}",
            delete(delete_device_profile),
        )
        .route("/api/device-files", get(get_device_files))
        .route(
            "/api/phrases",
            get(get_quick_phrases).post(save_quick_phrase),
        )
        .route("/api/phrases/{id}", delete(delete_quick_phrase))
        .route(
            "/api/text-templates",
            get(get_text_templates).post(save_text_template),
        )
        .route("/api/text-templates/{id}", delete(delete_text_template))
        .route("/api/adb-presets", get(get_adb_presets).post(save_adb_preset))
        .route("/api/adb-presets/{id}", delete(delete_adb_preset))
        .route("/api/adb/server", post(run_adb_server_command))
        .route("/api/adb/tcpip", post(run_adb_tcpip))
        .route("/api/adb/connect", post(run_adb_connect))
        .route("/api/adb/disconnect", post(run_adb_disconnect))
        .route("/api/adb/logcat", post(run_logcat))
        .route("/api/adb/logcat/clear", post(clear_logcat))
        .route("/api/uploads/apk", post(upload_apk))
        .route("/api/uploads/apks", get(get_uploaded_apks))
        .route("/api/uploads/apks/{file_name}", delete(delete_uploaded_apk))
        .route("/api/materials", get(get_materials))
        .route("/api/materials/upload", post(upload_material))
        .route("/api/materials/{id}", delete(delete_material))
        .route("/api/accounts", get(get_accounts).post(save_account))
        .route("/api/accounts/{id}", delete(delete_account))
        .route("/api/task-queue", get(get_queue).post(save_queue_item))
        .route("/api/task-queue/{id}", delete(delete_queue_item))
        .route("/api/task-queue/{id}/run", post(run_queue_item))
        .route("/api/groups", get(get_groups).post(save_group))
        .route("/api/groups/{name}", delete(delete_group))
        .route("/api/tasks", get(get_tasks).post(save_task_template))
        .route("/api/tasks/{id}", delete(delete_task_template))
        .route("/api/tasks/{id}/run", post(run_task_template))
        .route("/api/recordings", get(get_recordings).post(save_recording))
        .route("/api/recordings/draft/run", post(run_draft_recording))
        .route("/api/recordings/{id}", delete(delete_recording))
        .route("/api/recordings/{id}/run", post(run_recording))
        .route("/api/actions", post(run_action))
        .route("/api/fast-input", post(fast_input))
        .route("/api/text-dispatch", post(run_text_dispatch))
        .route(
            "/api/focus-mirror",
            get(get_focus_mirror)
                .post(start_focus_mirror)
                .delete(stop_focus_mirror),
        )
        .route(
            "/api/whep-bridge",
            get(get_whep_bridge)
                .post(start_whep_bridge)
                .delete(stop_whep_bridge),
        )
        .route("/api/screenshots", get(get_screenshots))
        .route("/api/screenshots/bundle", post(bundle_screenshots))
        .route("/api/screenshots/files/{file_name}", get(get_screenshot_file))
        .route("/api/screenshots/bundles/{file_name}", get(get_screenshot_bundle_file))
        .route("/api/previews/refresh", post(request_preview_refresh))
        .route("/api/preview-events", get(stream_preview_events))
        .route("/api/preview-stream/{serial}", get(stream_preview))
        .route("/api/previews/{serial}", get(get_preview))
        .route("/api/hosts", get(get_hosts).post(save_host))
        .route("/api/hosts/{id}", axum::routing::put(update_host).delete(delete_host_handler))
        .route("/api/hosts/{id}/test", post(test_host_connection))
        .route("/api/hosts/{id}/vms", get(get_vms).post(create_vms_handler))
        .route("/api/hosts/{id}/vms/{name}/start", post(start_vm))
        .route("/api/hosts/{id}/vms/{name}/stop", post(stop_vm))
        .route("/api/hosts/{id}/vms/{name}/restart", post(restart_vm))
        .route("/api/hosts/{id}/vms/{name}", delete(delete_vm_handler))
        .route("/api/hosts/{id}/vms/batch", post(batch_vm_action_handler))
        .route("/api/hosts/{id}/rotate", post(rotate_vms_handler))
        .route("/api/hosts/{id}/vms/{name}/provision", post(provision_vm_handler))
        .route("/api/profiles/stats", get(profile_stats_handler))
        .route("/api/profiles/generate", post(generate_profiles_handler))
        .route("/api/hosts/{id}/stats", get(get_host_stats))
        .route("/api/hosts/{id}/stats/stream", get(stream_host_stats))
        .route("/api/interactive/sync", get(ws_interactive_sync))
        .route("/api/interactive/{serial}", get(ws_interactive))
        .route("/api/scrcpy/{serial}", get(ws_scrcpy))
        .route("/api/scrcpy/{serial}/fps", get(get_scrcpy_fps))
        .route("/api/scrcpy/group-control", post(group_control))
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "http://127.0.0.1:5173".parse::<HeaderValue>().unwrap(),
                    "http://localhost:5173".parse::<HeaderValue>().unwrap(),
                    "http://127.0.0.1:8876".parse::<HeaderValue>().unwrap(),
                    "http://localhost:8876".parse::<HeaderValue>().unwrap(),
                    "tauri://localhost".parse::<HeaderValue>().unwrap(),
                    "https://tauri.localhost".parse::<HeaderValue>().unwrap(),
                ])
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
        )
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse().expect("invalid address");
    println!("Rust web API listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusPayload> {
    run_blocking(move || {
        let env = adb::detect_environment();
        let device_count = adb::list_devices()
            .map(|devices| devices.len())
            .unwrap_or(0);
        let (focus_mirror_running, focus_mirror_serial) = current_focus_mirror(&state);
        Ok(Json(StatusPayload {
            adb_available: env.adb_path.is_some(),
            adb_path: env.adb_path.map(|path| path.display().to_string()),
            scrcpy_available: env.scrcpy_path.is_some(),
            scrcpy_path: env.scrcpy_path.map(|path| path.display().to_string()),
            device_count,
            focus_mirror_serial,
            focus_mirror_running,
        }))
    })
    .await
    .unwrap_or_else(|error| {
        Json(StatusPayload {
            adb_available: false,
            adb_path: Some(error.message),
            scrcpy_available: false,
            scrcpy_path: None,
            device_count: 0,
            focus_mirror_serial: None,
            focus_mirror_running: false,
        })
    })
}

async fn get_devices(State(state): State<Arc<AppState>>) -> Result<Json<DevicesPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let devices = merge_device_profiles(
            adb::list_devices().map_err(ApiError::bad_request)?,
            &storage::load_device_profiles(&base_dir),
        );
        // 后台预热在线设备的 Shell 连接池（不阻塞响应）
        let online_serials: Vec<String> = devices
            .iter()
            .filter(|d| d.state == "device")
            .map(|d| d.serial.clone())
            .collect();
        if !online_serials.is_empty() {
            std::thread::spawn(move || adb::warm_shell_pool(&online_serials));
        }
        Ok(Json(DevicesPayload { devices }))
    })
    .await
}

async fn get_device_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DeviceProfilesPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(DeviceProfilesPayload {
            profiles: storage::load_device_profiles(&base_dir),
        }))
    })
    .await
}

async fn get_quick_phrases(
    State(state): State<Arc<AppState>>,
) -> Result<Json<QuickPhrasesPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(QuickPhrasesPayload {
            phrases: storage::load_quick_phrases(&base_dir),
        }))
    })
    .await
}

async fn get_text_templates(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TextTemplatesPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(TextTemplatesPayload {
            templates: storage::load_text_templates(&base_dir),
        }))
    })
    .await
}

async fn get_adb_presets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AdbPresetsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(AdbPresetsPayload {
            presets: storage::load_adb_presets(&base_dir),
        }))
    })
    .await
}

async fn get_device_files(
    Query(query): Query<DeviceFilesQuery>,
) -> Result<Json<DeviceFilesPayload>, ApiError> {
    let serial = query.serial.trim().to_string();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    let path = query
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/sdcard/Download")
        .to_string();
    run_blocking(move || {
        let entries = adb::list_remote_files(&serial, &path).map_err(ApiError::bad_request)?;
        Ok(Json(DeviceFilesPayload {
            serial,
            path,
            entries,
        }))
    })
    .await
}

async fn request_preview_refresh(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PreviewRefreshRequest>,
) -> Json<PreviewRefreshPayload> {
    let serials = payload
        .serials
        .into_iter()
        .map(|serial| serial.trim().to_string())
        .filter(|serial| !serial.is_empty())
        .collect::<Vec<_>>();
    let refreshed = serials.len();
    if refreshed > 0 {
        state
            .preview_hub
            .request_refresh_with_priority(serials, preview_priority_from_str(payload.priority.as_deref()));
    }
    Json(PreviewRefreshPayload {
        refreshed,
        last_updated_ms: state.preview_hub.last_updated_ms(),
    })
}

async fn get_groups(State(state): State<Arc<AppState>>) -> Result<Json<GroupsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(GroupsPayload {
            groups: storage::load_groups(&base_dir),
        }))
    })
    .await
}

async fn get_tasks(State(state): State<Arc<AppState>>) -> Result<Json<TasksPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(TasksPayload {
            templates: storage::load_task_templates(&base_dir),
            runs: storage::load_task_runs(&base_dir),
        }))
    })
    .await
}

async fn get_recordings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RecordingsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(RecordingsPayload {
            recordings: storage::load_recordings(&base_dir),
        }))
    })
    .await
}

async fn get_materials(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MaterialsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(MaterialsPayload {
            materials: storage::load_materials(&base_dir),
            directory: storage::material_dir(&base_dir).display().to_string(),
        }))
    })
    .await
}

async fn get_accounts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AccountsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(AccountsPayload {
            accounts: storage::load_accounts(&base_dir),
        }))
    })
    .await
}

async fn get_queue(State(state): State<Arc<AppState>>) -> Result<Json<QueuePayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        Ok(Json(QueuePayload {
            items: storage::load_queue(&base_dir),
        }))
    })
    .await
}

async fn get_screenshots(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ScreenshotsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let screenshot_dir = storage::screenshot_dir(&base_dir);
        let mut screenshots = std::fs::read_dir(&screenshot_dir)
            .map_err(|err| ApiError::bad_request(err.to_string()))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                if !metadata.is_file() {
                    return None;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                let extension = entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if extension != "png" && extension != "jpg" && extension != "jpeg" {
                    return None;
                }
                let serial = file_name
                    .split('-')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let created_at = metadata
                    .modified()
                    .ok()
                    .map(storage::format_system_time)
                    .unwrap_or_default();
                Some(ScreenshotItem {
                    file_name: file_name.clone(),
                    serial,
                    created_at,
                    size_bytes: metadata.len(),
                    image_url: format!("/api/screenshots/files/{file_name}"),
                })
            })
            .collect::<Vec<_>>();
        screenshots.sort_by(|left, right| right.file_name.cmp(&left.file_name));
        Ok(Json(ScreenshotsPayload {
            screenshots,
            directory: screenshot_dir.display().to_string(),
        }))
    })
    .await
}

async fn bundle_screenshots(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ScreenshotBundlePayload>, ApiError> {
    let screenshot_dir = storage::screenshot_dir(&state.base_dir);
    let export_dir = storage::export_dir(&state.base_dir);
    std::fs::create_dir_all(&export_dir)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;

    let has_screenshots = std::fs::read_dir(&screenshot_dir)
        .map_err(|err| ApiError::bad_request(err.to_string()))?
        .filter_map(Result::ok)
        .any(|entry| entry.metadata().map(|meta| meta.is_file()).unwrap_or(false));
    if !has_screenshots {
        return Err(ApiError::bad_request("当前还没有可打包的截图"));
    }

    let file_name = format!("screenshots-{}.zip", storage::timestamp_label());
    let artifact_path = export_dir.join(&file_name);
    let status = Command::new("ditto")
        .arg("-c")
        .arg("-k")
        .arg("--sequesterRsrc")
        .arg("--keepParent")
        .arg(&screenshot_dir)
        .arg(&artifact_path)
        .status()
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    if !status.success() {
        return Err(ApiError::bad_request("截图打包失败"));
    }

    Ok(Json(ScreenshotBundlePayload {
        file_name: file_name.clone(),
        artifact_path: artifact_path.display().to_string(),
        download_url: format!("/api/screenshots/bundles/{file_name}"),
    }))
}

async fn get_screenshot_file(
    State(state): State<Arc<AppState>>,
    Path(file_name): Path<String>,
) -> Result<Response, ApiError> {
    serve_local_file(storage::screenshot_dir(&state.base_dir), &file_name)
}

async fn get_screenshot_bundle_file(
    State(state): State<Arc<AppState>>,
    Path(file_name): Path<String>,
) -> Result<Response, ApiError> {
    serve_local_file(storage::export_dir(&state.base_dir), &file_name)
}

async fn save_group(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveGroupRequest>,
) -> Result<Json<GroupsPayload>, ApiError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("分组名不能为空"));
    }
    let mut groups = storage::load_groups(&state.base_dir);
    groups.retain(|group| group.name != name);
    groups.push(DeviceGroup {
        name: name.to_string(),
        serials: payload
            .serials
            .into_iter()
            .filter(|serial| !serial.trim().is_empty())
            .collect(),
    });
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    storage::save_groups(&state.base_dir, &groups).map_err(ApiError::bad_request)?;
    Ok(Json(GroupsPayload { groups }))
}

async fn save_device_profile(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveDeviceProfileRequest>,
) -> Result<Json<DeviceProfilesPayload>, ApiError> {
    let serial = payload.serial.trim();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }

    let mut profiles = storage::load_device_profiles(&state.base_dir);
    profiles.retain(|profile| profile.serial != serial);

    let profile = DeviceProfile {
        serial: serial.to_string(),
        custom_name: payload.custom_name.trim().to_string(),
        custom_number: payload.custom_number.trim().to_string(),
        remark: payload.remark.trim().to_string(),
        tags: payload
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .fold(Vec::<String>::new(), |mut acc, tag| {
                if !acc.contains(&tag) {
                    acc.push(tag);
                }
                acc
            }),
    };

    if !profile.custom_name.is_empty()
        || !profile.custom_number.is_empty()
        || !profile.remark.is_empty()
        || !profile.tags.is_empty()
    {
        profiles.push(profile);
        profiles.sort_by(|a, b| a.serial.cmp(&b.serial));
    }

    storage::save_device_profiles(&state.base_dir, &profiles).map_err(ApiError::bad_request)?;
    Ok(Json(DeviceProfilesPayload { profiles }))
}

async fn save_quick_phrase(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveQuickPhraseRequest>,
) -> Result<Json<QuickPhrasesPayload>, ApiError> {
    let name = payload.name.trim();
    let group = payload.group.trim();
    let text = payload.text.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("短语名不能为空"));
    }
    if text.is_empty() {
        return Err(ApiError::bad_request("短语内容不能为空"));
    }

    let mut phrases = storage::load_quick_phrases(&state.base_dir);
    phrases.retain(|phrase| !(phrase.name == name && phrase.group == group));
    phrases.push(QuickPhrase {
        id: storage::make_id("phrase"),
        name: name.to_string(),
        group: group.to_string(),
        text: text.to_string(),
        created_at: storage::timestamp_label(),
    });
    phrases.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    storage::save_quick_phrases(&state.base_dir, &phrases).map_err(ApiError::bad_request)?;
    Ok(Json(QuickPhrasesPayload { phrases }))
}

async fn save_text_template(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveTextTemplateRequest>,
) -> Result<Json<TextTemplatesPayload>, ApiError> {
    let name = payload.name.trim();
    let group = payload.group.trim();
    let template = payload.template.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("模板名不能为空"));
    }
    if template.is_empty() {
        return Err(ApiError::bad_request("模板内容不能为空"));
    }

    let mut templates = storage::load_text_templates(&state.base_dir);
    templates.retain(|item| !(item.name == name && item.group == group));
    templates.push(TextTemplatePreset {
        id: storage::make_id("text-template"),
        name: name.to_string(),
        group: group.to_string(),
        template: template.to_string(),
        created_at: storage::timestamp_label(),
    });
    templates.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    storage::save_text_templates(&state.base_dir, &templates).map_err(ApiError::bad_request)?;
    Ok(Json(TextTemplatesPayload { templates }))
}

async fn save_adb_preset(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveAdbPresetRequest>,
) -> Result<Json<AdbPresetsPayload>, ApiError> {
    let name = payload.name.trim();
    let group = payload.group.trim();
    let command = payload.command.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("ADB 命令名不能为空"));
    }
    if command.is_empty() {
        return Err(ApiError::bad_request("ADB 命令内容不能为空"));
    }

    let mut presets = storage::load_adb_presets(&state.base_dir);
    presets.retain(|item| !(item.name == name && item.group == group));
    presets.push(AdbCommandPreset {
        id: storage::make_id("adb-preset"),
        name: name.to_string(),
        group: group.to_string(),
        command: command.to_string(),
        created_at: storage::timestamp_label(),
    });
    presets.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    storage::save_adb_presets(&state.base_dir, &presets).map_err(ApiError::bad_request)?;
    Ok(Json(AdbPresetsPayload { presets }))
}

async fn delete_group(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<GroupsPayload>, ApiError> {
    let mut groups = storage::load_groups(&state.base_dir);
    groups.retain(|group| group.name != name);
    storage::save_groups(&state.base_dir, &groups).map_err(ApiError::bad_request)?;
    Ok(Json(GroupsPayload { groups }))
}

async fn delete_device_profile(
    State(state): State<Arc<AppState>>,
    Path(serial): Path<String>,
) -> Result<Json<DeviceProfilesPayload>, ApiError> {
    let mut profiles = storage::load_device_profiles(&state.base_dir);
    profiles.retain(|profile| profile.serial != serial);
    storage::save_device_profiles(&state.base_dir, &profiles).map_err(ApiError::bad_request)?;
    Ok(Json(DeviceProfilesPayload { profiles }))
}

async fn delete_quick_phrase(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<QuickPhrasesPayload>, ApiError> {
    let mut phrases = storage::load_quick_phrases(&state.base_dir);
    phrases.retain(|phrase| phrase.id != id);
    storage::save_quick_phrases(&state.base_dir, &phrases).map_err(ApiError::bad_request)?;
    Ok(Json(QuickPhrasesPayload { phrases }))
}

async fn delete_adb_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AdbPresetsPayload>, ApiError> {
    let mut presets = storage::load_adb_presets(&state.base_dir);
    presets.retain(|item| item.id != id);
    storage::save_adb_presets(&state.base_dir, &presets).map_err(ApiError::bad_request)?;
    Ok(Json(AdbPresetsPayload { presets }))
}

async fn upload_apk(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<Json<ApkUploadPayload>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::bad_request("上传内容为空"));
    }
    let raw_name = headers
        .get("x-file-name")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("upload.apk");
    let safe_name = sanitize_upload_name(raw_name)?;
    let file_name = format!("{}-{}", storage::timestamp_label(), safe_name);
    let target_path = storage::upload_dir(&state.base_dir).join(&file_name);
    std::fs::write(&target_path, &body).map_err(|err| ApiError::bad_request(err.to_string()))?;
    let inspect = inspect_apk_metadata(&target_path);
    upsert_apk_metadata_cache_entry(
        &state.base_dir,
        build_apk_metadata_cache_entry(&target_path, body.len() as u64, &inspect),
    )
    .map_err(ApiError::bad_request)?;
    Ok(Json(ApkUploadPayload {
        file_name,
        artifact_path: target_path.display().to_string(),
        size_bytes: body.len(),
        package_name: inspect.package_name,
        version_name: inspect.version_name,
        version_code: inspect.version_code,
        inspect_error: inspect.inspect_error,
    }))
}

async fn get_uploaded_apks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApkUploadsPayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let upload_dir = storage::upload_dir(&base_dir);
        let mut metadata_cache = storage::load_apk_metadata_cache(&base_dir);
        let mut cache_changed = false;
        let mut apks = std::fs::read_dir(&upload_dir)
            .map_err(|err| ApiError::bad_request(err.to_string()))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                if !metadata.is_file() {
                    return None;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                if !file_name.to_ascii_lowercase().ends_with(".apk") {
                    return None;
                }
                let path = entry.path();
                let created_at = metadata
                    .modified()
                    .ok()
                    .map(storage::format_system_time)
                    .unwrap_or_default();
                let modified_at_ms = metadata
                    .modified()
                    .ok()
                    .map(system_time_millis)
                    .unwrap_or_default();
                let size_bytes = metadata.len();
                let inspect = resolve_apk_metadata(
                    &base_dir,
                    &path,
                    size_bytes,
                    modified_at_ms,
                    &mut metadata_cache,
                    &mut cache_changed,
                );
                Some(UploadedApkItem {
                    file_name,
                    artifact_path: path.display().to_string(),
                    size_bytes,
                    created_at,
                    package_name: inspect.package_name,
                    version_name: inspect.version_name,
                    version_code: inspect.version_code,
                    inspect_error: inspect.inspect_error,
                })
            })
            .collect::<Vec<_>>();
        apks.sort_by(|left, right| right.file_name.cmp(&left.file_name));
        let upload_prefix = upload_dir.display().to_string();
        let previous_cache_len = metadata_cache.len();
        metadata_cache.retain(|item| {
            item.artifact_path.starts_with(&upload_prefix)
                && std::path::Path::new(&item.artifact_path).exists()
        });
        if metadata_cache.len() != previous_cache_len {
            cache_changed = true;
        }
        if cache_changed {
            storage::save_apk_metadata_cache(&base_dir, &metadata_cache)
                .map_err(ApiError::bad_request)?;
        }
        Ok(Json(ApkUploadsPayload {
            apks,
            directory: upload_dir.display().to_string(),
        }))
    })
    .await
}

async fn delete_uploaded_apk(
    State(state): State<Arc<AppState>>,
    Path(file_name): Path<String>,
) -> Result<Json<ApkUploadsPayload>, ApiError> {
    let safe_name = sanitize_upload_name(&file_name)?;
    let path = storage::upload_dir(&state.base_dir).join(&safe_name);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|err| ApiError::bad_request(err.to_string()))?;
    }
    let artifact_path = path.display().to_string();
    let mut cache = storage::load_apk_metadata_cache(&state.base_dir);
    let original_len = cache.len();
    cache.retain(|item| item.artifact_path != artifact_path);
    if cache.len() != original_len {
        storage::save_apk_metadata_cache(&state.base_dir, &cache).map_err(ApiError::bad_request)?;
    }
    get_uploaded_apks(State(state)).await
}

async fn upload_material(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<Json<MaterialsPayload>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::bad_request("上传内容为空"));
    }
    let raw_name = headers
        .get("x-file-name")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("material.bin");
    let safe_name = sanitize_file_name(raw_name)?.replace(' ', "_");
    let file_name = format!("{}-{}", storage::timestamp_label(), safe_name);
    let target_path = storage::material_dir(&state.base_dir).join(&file_name);
    std::fs::write(&target_path, &body).map_err(|err| ApiError::bad_request(err.to_string()))?;

    let mut materials = storage::load_materials(&state.base_dir);
    materials.insert(
        0,
        MaterialAsset {
            id: storage::make_id("material"),
            name: raw_name.to_string(),
            file_name,
            artifact_path: target_path.display().to_string(),
            kind: infer_material_kind(raw_name),
            tags: Vec::new(),
            note: String::new(),
            size_bytes: body.len() as u64,
            created_at: storage::timestamp_label(),
        },
    );
    storage::save_materials(&state.base_dir, &materials).map_err(ApiError::bad_request)?;
    Ok(Json(MaterialsPayload {
        materials,
        directory: storage::material_dir(&state.base_dir).display().to_string(),
    }))
}

async fn delete_material(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<MaterialsPayload>, ApiError> {
    let mut materials = storage::load_materials(&state.base_dir);
    if let Some(item) = materials.iter().find(|item| item.id == id) {
        let path = PathBuf::from(&item.artifact_path);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
    materials.retain(|item| item.id != id);
    storage::save_materials(&state.base_dir, &materials).map_err(ApiError::bad_request)?;
    Ok(Json(MaterialsPayload {
        materials,
        directory: storage::material_dir(&state.base_dir).display().to_string(),
    }))
}

async fn save_account(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveAccountRequest>,
) -> Result<Json<AccountsPayload>, ApiError> {
    let name = payload.account_name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("账号名不能为空"));
    }
    let serial = payload.serial.trim();
    if serial.is_empty() {
        return Err(ApiError::bad_request("先选中一台设备再绑定账号"));
    }
    let mut accounts = storage::load_accounts(&state.base_dir);
    accounts.retain(|item| item.account_name != name || item.serial != serial);
    accounts.insert(
        0,
        AccountBinding {
            id: storage::make_id("account"),
            platform: payload.platform.trim().to_string(),
            account_name: name.to_string(),
            account_group: payload.account_group.trim().to_string(),
            status: payload.status.trim().to_string(),
            serial: serial.to_string(),
            note: payload.note.trim().to_string(),
            created_at: storage::timestamp_label(),
        },
    );
    storage::save_accounts(&state.base_dir, &accounts).map_err(ApiError::bad_request)?;
    Ok(Json(AccountsPayload { accounts }))
}

async fn delete_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AccountsPayload>, ApiError> {
    let mut accounts = storage::load_accounts(&state.base_dir);
    accounts.retain(|item| item.id != id);
    storage::save_accounts(&state.base_dir, &accounts).map_err(ApiError::bad_request)?;
    Ok(Json(AccountsPayload { accounts }))
}

async fn save_queue_item(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveQueueItemRequest>,
) -> Result<Json<QueuePayload>, ApiError> {
    let name = payload.name.trim();
    let kind = payload.kind.trim();
    let ref_id = payload.ref_id.trim();
    if name.is_empty() || kind.is_empty() || ref_id.is_empty() {
        return Err(ApiError::bad_request("队列项名称、类型和引用不能为空"));
    }
    let serials = payload
        .serials
        .into_iter()
        .map(|serial| serial.trim().to_string())
        .filter(|serial| !serial.is_empty())
        .collect::<Vec<_>>();
    if serials.is_empty() {
        return Err(ApiError::bad_request("先选中至少一台设备再加入任务队列"));
    }
    let mut items = storage::load_queue(&state.base_dir);
    items.insert(
        0,
        QueueItem {
            id: storage::make_id("queue"),
            name: name.to_string(),
            kind: kind.to_string(),
            ref_id: ref_id.to_string(),
            serials,
            status: "idle".to_string(),
            note: payload.note.trim().to_string(),
            created_at: storage::timestamp_label(),
            last_run_at: String::new(),
        },
    );
    storage::save_queue(&state.base_dir, &items).map_err(ApiError::bad_request)?;
    Ok(Json(QueuePayload { items }))
}

async fn delete_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<QueuePayload>, ApiError> {
    let mut items = storage::load_queue(&state.base_dir);
    items.retain(|item| item.id != id);
    storage::save_queue(&state.base_dir, &items).map_err(ApiError::bad_request)?;
    Ok(Json(QueuePayload { items }))
}

async fn run_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut items = storage::load_queue(&base_dir);
        let queue_item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| ApiError::bad_request("没有找到对应队列项"))?;
        if queue_item.serials.is_empty() {
            return Err(ApiError::bad_request("这个队列项还没有绑定设备"));
        }

        let response = match queue_item.kind.as_str() {
            "recording" => {
                let recordings = storage::load_recordings(&base_dir);
                let recording = recordings
                    .into_iter()
                    .find(|item| item.id == queue_item.ref_id)
                    .ok_or_else(|| ApiError::bad_request("没有找到对应录制"))?;
                run_recording_steps(
                    &base_dir,
                    queue_item.serials.clone(),
                    &queue_item.name,
                    Some(queue_item.ref_id.clone()),
                    &recording.steps,
                )?
            }
            _ => {
                let templates = storage::load_task_templates(&base_dir);
                let template = templates
                    .into_iter()
                    .find(|item| item.id == queue_item.ref_id)
                    .ok_or_else(|| ApiError::bad_request("没有找到对应任务模板"))?;
                let action = build_action(&template.action, &template.params)?;
                let results = adb::run_batch(
                    queue_item.serials.clone(),
                    action,
                    storage::screenshot_dir(&base_dir),
                )
                .into_iter()
                .map(|result| ActionResultPayload {
                    serial: result.serial,
                    label: result.label,
                    ok: result.ok,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    artifact_path: result.artifact_path.map(|path| path.display().to_string()),
                    duration_ms: result.duration_ms,
                })
                .collect::<Vec<_>>();
                persist_run_record(
                    &base_dir,
                    Some(queue_item.name.clone()),
                    template.action,
                    queue_item.serials.clone(),
                    &results,
                    Some(queue_item.ref_id.clone()),
                );
                ActionResponse { results }
            }
        };

        let success_count = response.results.iter().filter(|result| result.ok).count();
        let failure_count = response.results.len().saturating_sub(success_count);
        queue_item.status = if response.results.is_empty() || success_count == 0 {
            "failed".to_string()
        } else if failure_count == 0 {
            "completed".to_string()
        } else {
            "partial".to_string()
        };
        queue_item.last_run_at = storage::timestamp_label();
        storage::save_queue(&base_dir, &items).map_err(ApiError::bad_request)?;
        Ok(Json(response))
    })
    .await
}

async fn delete_text_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TextTemplatesPayload>, ApiError> {
    let mut templates = storage::load_text_templates(&state.base_dir);
    templates.retain(|item| item.id != id);
    storage::save_text_templates(&state.base_dir, &templates).map_err(ApiError::bad_request)?;
    Ok(Json(TextTemplatesPayload { templates }))
}

async fn save_task_template(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveTaskTemplateRequest>,
) -> Result<Json<TasksPayload>, ApiError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("任务名不能为空"));
    }
    let action = payload.action.trim();
    if action.is_empty() {
        return Err(ApiError::bad_request("动作不能为空"));
    }

    let mut templates = storage::load_task_templates(&state.base_dir);
    templates.retain(|template| template.name != name);
    templates.push(TaskTemplate {
        id: storage::make_id("task"),
        name: name.to_string(),
        action: action.to_string(),
        params: payload.params,
        created_at: storage::timestamp_label(),
    });
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    storage::save_task_templates(&state.base_dir, &templates).map_err(ApiError::bad_request)?;

    Ok(Json(TasksPayload {
        templates,
        runs: storage::load_task_runs(&state.base_dir),
    }))
}

async fn delete_task_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TasksPayload>, ApiError> {
    let mut templates = storage::load_task_templates(&state.base_dir);
    templates.retain(|template| template.id != id);
    storage::save_task_templates(&state.base_dir, &templates).map_err(ApiError::bad_request)?;
    Ok(Json(TasksPayload {
        templates,
        runs: storage::load_task_runs(&state.base_dir),
    }))
}

async fn save_recording(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveRecordingRequest>,
) -> Result<Json<RecordingsPayload>, ApiError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("录制名不能为空"));
    }
    if payload.steps.is_empty() {
        return Err(ApiError::bad_request("至少录制一个动作"));
    }

    let mut recordings = storage::load_recordings(&state.base_dir);
    recordings.retain(|recording| recording.name != name);
    recordings.push(RecordingTemplate {
        id: storage::make_id("recording"),
        name: name.to_string(),
        steps: payload.steps,
        created_at: storage::timestamp_label(),
    });
    recordings.sort_by(|a, b| a.name.cmp(&b.name));
    storage::save_recordings(&state.base_dir, &recordings).map_err(ApiError::bad_request)?;
    Ok(Json(RecordingsPayload { recordings }))
}

async fn delete_recording(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RecordingsPayload>, ApiError> {
    let mut recordings = storage::load_recordings(&state.base_dir);
    recordings.retain(|recording| recording.id != id);
    storage::save_recordings(&state.base_dir, &recordings).map_err(ApiError::bad_request)?;
    Ok(Json(RecordingsPayload { recordings }))
}

async fn run_action(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ActionRequest>,
) -> Result<Json<ActionResponse>, ApiError> {
    if payload.serials.is_empty() {
        return Err(ApiError::bad_request("至少选择一台设备"));
    }
    let action = build_action(&payload.action, &payload.params)?;
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let results = adb::run_batch(
            payload.serials.clone(),
            action,
            storage::screenshot_dir(&base_dir),
        )
        .into_iter()
        .map(|result| ActionResultPayload {
            serial: result.serial,
            label: result.label,
            ok: result.ok,
            stdout: result.stdout,
            stderr: result.stderr,
            artifact_path: result.artifact_path.map(|path| path.display().to_string()),
            duration_ms: result.duration_ms,
        })
        .collect::<Vec<_>>();

        persist_run_record(
            &base_dir,
            payload.task_name,
            payload.action,
            payload.serials,
            &results,
            payload.template_id,
        );

        Ok(Json(ActionResponse { results }))
    })
    .await
}

async fn fast_input(
    Json(payload): Json<FastInputRequest>,
) -> Result<Json<Value>, ApiError> {
    let serial = payload.serial.trim().to_string();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }

    let cmd = match payload.action.trim() {
        "tap" => {
            let x = required_i32(&payload.params, "x")?;
            let y = required_i32(&payload.params, "y")?;
            format!("input tap {x} {y}")
        }
        "swipe" => {
            let x1 = required_i32(&payload.params, "x1")?;
            let y1 = required_i32(&payload.params, "y1")?;
            let x2 = required_i32(&payload.params, "x2")?;
            let y2 = required_i32(&payload.params, "y2")?;
            let dur = optional_i32(&payload.params, "duration").unwrap_or(300);
            format!("input swipe {x1} {y1} {x2} {y2} {dur}")
        }
        "text" => {
            let text = required_string(&payload.params, "text")?;
            let result = tokio::task::spawn_blocking(move || {
                let results = adb::run_batch(
                    vec![serial],
                    BatchAction::Text(text),
                    std::path::PathBuf::from("/tmp"),
                );
                results.into_iter().next()
            })
            .await
            .map_err(|e| ApiError::bad_request(format!("执行失败: {e}")))?;
            return match result {
                Some(r) if r.ok => Ok(Json(serde_json::json!({ "ok": true, "output": r.stdout }))),
                Some(r) => Ok(Json(serde_json::json!({ "ok": false, "error": r.stderr }))),
                None => Ok(Json(serde_json::json!({ "ok": false, "error": "无结果" }))),
            };
        }
        "keyevent" => {
            let key = required_string(&payload.params, "key")?;
            let normalized = if key.starts_with("KEYCODE_") || key.chars().all(|c| c.is_ascii_digit()) {
                key
            } else {
                format!("KEYCODE_{}", key.to_uppercase())
            };
            format!("input keyevent {normalized}")
        }
        other => return Err(ApiError::bad_request(&format!("fast-input 不支持: {other}"))),
    };

    let result = tokio::task::spawn_blocking(move || adb::fast_shell_command(&serial, &cmd))
        .await
        .map_err(|e| ApiError::bad_request(format!("执行失败: {e}")))?;

    match result {
        Ok(output) => Ok(Json(serde_json::json!({ "ok": true, "output": output }))),
        Err(msg) => Ok(Json(serde_json::json!({ "ok": false, "error": msg }))),
    }
}

async fn run_text_dispatch(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TextDispatchRequest>,
) -> Result<Json<ActionResponse>, ApiError> {
    if payload.entries.is_empty() {
        return Err(ApiError::bad_request("至少分发一条文本"));
    }

    let mode = payload.mode.trim().to_lowercase();
    let action_name = match mode.as_str() {
        "clipboard" => "clipboard",
        _ => "text_dispatch",
    };
    let screenshot_dir = storage::screenshot_dir(&state.base_dir);

    let entries: Vec<(String, String)> = payload.entries
        .into_iter()
        .filter_map(|entry| {
            let serial = entry.serial.trim().to_string();
            let text = entry.text.trim().to_string();
            if serial.is_empty() || text.is_empty() { None }
            else { Some((serial, text)) }
        })
        .collect();

    if entries.is_empty() {
        return Err(ApiError::bad_request("没有可执行的文本分发项"));
    }

    let serials = entries.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
    let mut results = Vec::new();
    let max_concurrent = 12;

    for chunk in entries.chunks(max_concurrent) {
        let mut handles = Vec::new();
        for (serial, text) in chunk {
            let serial = serial.clone();
            let text = text.clone();
            let action = if mode == "clipboard" {
                BatchAction::Clipboard(text)
            } else {
                BatchAction::Text(text)
            };
            let dir = screenshot_dir.clone();
            handles.push(std::thread::spawn(move || {
                adb::run_batch(vec![serial], action, dir)
            }));
        }
        for handle in handles {
            if let Ok(items) = handle.join() {
                results.extend(items.into_iter().map(|result| ActionResultPayload {
                    serial: result.serial,
                    label: result.label,
                    ok: result.ok,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    artifact_path: result.artifact_path.map(|path| path.display().to_string()),
                    duration_ms: result.duration_ms,
                }));
            }
        }
    }
    results.sort_by(|a, b| a.serial.cmp(&b.serial));

    persist_run_record(
        &state.base_dir,
        payload.task_name,
        action_name.to_string(),
        serials,
        &results,
        None,
    );

    Ok(Json(ActionResponse { results }))
}

async fn run_task_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<RunTaskTemplateRequest>,
) -> Result<Json<ActionResponse>, ApiError> {
    if payload.serials.is_empty() {
        return Err(ApiError::bad_request("至少选择一台设备"));
    }
    let templates = storage::load_task_templates(&state.base_dir);
    let template = templates
        .into_iter()
        .find(|template| template.id == id)
        .ok_or_else(|| ApiError::bad_request("没有找到对应任务模板"))?;
    let action = build_action(&template.action, &template.params)?;
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let results = adb::run_batch(
            payload.serials.clone(),
            action,
            storage::screenshot_dir(&base_dir),
        )
        .into_iter()
        .map(|result| ActionResultPayload {
            serial: result.serial,
            label: result.label,
            ok: result.ok,
            stdout: result.stdout,
            stderr: result.stderr,
            artifact_path: result.artifact_path.map(|path| path.display().to_string()),
            duration_ms: result.duration_ms,
        })
        .collect::<Vec<_>>();

        persist_run_record(
            &base_dir,
            Some(template.name),
            template.action,
            payload.serials,
            &results,
            Some(id),
        );

        Ok(Json(ActionResponse { results }))
    })
    .await
}

async fn run_recording(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<RunRecordingRequest>,
) -> Result<Json<ActionResponse>, ApiError> {
    if payload.serials.is_empty() {
        return Err(ApiError::bad_request("至少选择一台设备"));
    }
    let recordings = storage::load_recordings(&state.base_dir);
    let recording = recordings
        .into_iter()
        .find(|recording| recording.id == id)
        .ok_or_else(|| ApiError::bad_request("没有找到对应录制"))?;
    if recording.steps.is_empty() {
        return Err(ApiError::bad_request("录制里还没有动作"));
    }
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let response = run_recording_steps(
            &base_dir,
            payload.serials,
            &recording.name,
            Some(id),
            &recording.steps,
        )?;
        Ok(Json(response))
    })
    .await
}

async fn run_draft_recording(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RunDraftRecordingRequest>,
) -> Result<Json<ActionResponse>, ApiError> {
    if payload.serials.is_empty() {
        return Err(ApiError::bad_request("至少选择一台设备"));
    }
    if payload.steps.is_empty() {
        return Err(ApiError::bad_request("草稿里还没有动作"));
    }
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let response = run_recording_steps(
            &base_dir,
            payload.serials,
            payload.name.as_deref().unwrap_or("录制草稿"),
            None,
            &payload.steps,
        )?;
        Ok(Json(response))
    })
    .await
}

fn run_recording_steps(
    base_dir: &std::path::Path,
    serials: Vec<String>,
    name: &str,
    template_id: Option<String>,
    steps: &[RecordingStep],
) -> Result<ActionResponse, ApiError> {
    if steps.is_empty() {
        return Err(ApiError::bad_request("录制里还没有动作"));
    }
    let mut all_results = Vec::new();
    let mut device_ok = serials
        .iter()
        .map(|serial| (serial.clone(), true))
        .collect::<std::collections::BTreeMap<_, _>>();

    for (index, step) in steps.iter().enumerate() {
        if step.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(step.delay_ms));
        }
        let action = build_action(&step.action, &step.params)?;
        let step_results =
            adb::run_batch(serials.clone(), action, storage::screenshot_dir(base_dir))
                .into_iter()
                .map(|result| {
                    if !result.ok {
                        device_ok.insert(result.serial.clone(), false);
                    }
                    ActionResultPayload {
                        serial: result.serial,
                        label: format!("步骤 {} · {}", index + 1, result.label),
                        ok: result.ok,
                        stdout: result.stdout,
                        stderr: result.stderr,
                        artifact_path: result.artifact_path.map(|path| path.display().to_string()),
                        duration_ms: result.duration_ms,
                    }
                })
                .collect::<Vec<_>>();
        all_results.extend(step_results);
    }

    persist_task_run_record(
        base_dir,
        TaskRunRecord {
            id: storage::make_id("run"),
            name: name.to_string(),
            action: "recording".to_string(),
            created_at: storage::timestamp_label(),
            device_count: serials.len(),
            success_count: device_ok.values().filter(|ok| **ok).count(),
            failure_count: device_ok.values().filter(|ok| !**ok).count(),
            serials,
            template_id,
        },
    );

    Ok(ActionResponse {
        results: all_results,
    })
}

async fn get_focus_mirror(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FocusMirrorPayload>, ApiError> {
    let (running, serial) = current_focus_mirror(&state);
    Ok(Json(FocusMirrorPayload { running, serial }))
}

async fn start_focus_mirror(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FocusMirrorRequest>,
) -> Result<Json<FocusMirrorPayload>, ApiError> {
    let serial = payload.serial.trim();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    let mut mirror = state
        .focus_mirror
        .lock()
        .map_err(|_| ApiError::bad_request("实时画面状态锁定失败"))?;
    cleanup_focus_mirror(&mut mirror);
    if mirror.serial.as_deref() == Some(serial) && mirror.child.is_some() {
        return Ok(Json(FocusMirrorPayload {
            running: true,
            serial: Some(serial.to_string()),
        }));
    }
    stop_focus_mirror_locked(&mut mirror);
    let child = adb::spawn_mirror_process(serial).map_err(ApiError::bad_request)?;
    mirror.serial = Some(serial.to_string());
    mirror.child = Some(child);
    Ok(Json(FocusMirrorPayload {
        running: true,
        serial: Some(serial.to_string()),
    }))
}

async fn stop_focus_mirror(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FocusMirrorPayload>, ApiError> {
    let mut mirror = state
        .focus_mirror
        .lock()
        .map_err(|_| ApiError::bad_request("实时画面状态锁定失败"))?;
    stop_focus_mirror_locked(&mut mirror);
    Ok(Json(FocusMirrorPayload {
        running: false,
        serial: None,
    }))
}

async fn get_whep_bridge(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WhepBridgeQuery>,
) -> Result<Json<WhepBridgePayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    let serial = query.serial.trim().to_string();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    run_blocking(move || Ok(Json(inspect_whep_bridge(&base_dir, &serial)?))).await
}

async fn start_whep_bridge(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WhepBridgeRequest>,
) -> Result<Json<WhepBridgePayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    let serial = payload.serial.trim().to_string();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    run_blocking(move || {
        run_whep_bridge_command(&base_dir, "start", &serial)?;
        Ok(Json(inspect_whep_bridge(&base_dir, &serial)?))
    })
    .await
}

async fn stop_whep_bridge(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WhepBridgeQuery>,
) -> Result<Json<WhepBridgePayload>, ApiError> {
    let base_dir = state.base_dir.clone();
    let serial = query.serial.trim().to_string();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    run_blocking(move || {
        run_whep_bridge_command(&base_dir, "stop", &serial)?;
        Ok(Json(inspect_whep_bridge(&base_dir, &serial)?))
    })
    .await
}

async fn get_preview(
    Path(serial): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let trimmed = serial.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    state.preview_hub.touch_consumer();
    state
        .preview_hub
        .request_refresh_with_priority([trimmed.clone()], PreviewPriority::High);
    let preview = state
        .preview_hub
        .get_or_capture(&trimmed, 720, 62)
        .map_err(ApiError::bad_request)?;
    let mut response = Response::new(preview.jpeg.into());
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

async fn run_adb_server_command(
    Json(payload): Json<AdbServerRequest>,
) -> Result<Json<CommandResultPayload>, ApiError> {
    run_blocking(move || {
        let output = adb::adb_server_command(&payload.command).map_err(ApiError::bad_request)?;
        Ok(Json(CommandResultPayload {
            ok: output.status_ok,
            stdout: output.stdout,
            stderr: output.stderr,
        }))
    })
    .await
}

async fn run_adb_tcpip(
    Json(payload): Json<AdbTcpipRequest>,
) -> Result<Json<CommandResultPayload>, ApiError> {
    run_blocking(move || {
        let output = adb::adb_tcpip(&payload.serial, payload.port.unwrap_or(5555))
            .map_err(ApiError::bad_request)?;
        Ok(Json(CommandResultPayload {
            ok: output.status_ok,
            stdout: output.stdout,
            stderr: output.stderr,
        }))
    })
    .await
}

async fn run_adb_connect(
    Json(payload): Json<AdbConnectRequest>,
) -> Result<Json<CommandResultPayload>, ApiError> {
    run_blocking(move || {
        let output = adb::adb_connect(&payload.target).map_err(ApiError::bad_request)?;
        Ok(Json(CommandResultPayload {
            ok: output.status_ok,
            stdout: output.stdout,
            stderr: output.stderr,
        }))
    })
    .await
}

async fn run_adb_disconnect(
    Json(payload): Json<AdbDisconnectRequest>,
) -> Result<Json<CommandResultPayload>, ApiError> {
    run_blocking(move || {
        let output = adb::adb_disconnect(payload.target.as_deref()).map_err(ApiError::bad_request)?;
        Ok(Json(CommandResultPayload {
            ok: output.status_ok,
            stdout: output.stdout,
            stderr: output.stderr,
        }))
    })
    .await
}

async fn run_logcat(
    Json(payload): Json<LogcatRequest>,
) -> Result<Json<LogcatPayload>, ApiError> {
    let serials = payload
        .serials
        .into_iter()
        .map(|serial| serial.trim().to_string())
        .filter(|serial| !serial.is_empty())
        .collect::<Vec<_>>();
    if serials.is_empty() {
        return Err(ApiError::bad_request("至少选择一台设备"));
    }
    let lines = payload.lines.unwrap_or(200);
    let filter = payload.filter.clone();
    run_blocking(move || {
        let mut entries = Vec::new();
        for serial in serials {
            let output =
                adb::read_logcat(&serial, lines, filter.as_deref()).map_err(ApiError::bad_request)?;
            entries.push(LogcatEntryPayload {
                serial,
                ok: output.status_ok,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }
        Ok(Json(LogcatPayload { entries }))
    })
    .await
}

async fn clear_logcat(
    Json(payload): Json<LogcatRequest>,
) -> Result<Json<LogcatPayload>, ApiError> {
    let serials = payload
        .serials
        .into_iter()
        .map(|serial| serial.trim().to_string())
        .filter(|serial| !serial.is_empty())
        .collect::<Vec<_>>();
    if serials.is_empty() {
        return Err(ApiError::bad_request("至少选择一台设备"));
    }
    run_blocking(move || {
        let mut entries = Vec::new();
        for serial in serials {
            let output = adb::clear_logcat(&serial).map_err(ApiError::bad_request)?;
            entries.push(LogcatEntryPayload {
                serial,
                ok: output.status_ok,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }
        Ok(Json(LogcatPayload { entries }))
    })
    .await
}

async fn stream_preview(
    Path(serial): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let serial = serial.trim().to_string();
    if serial.is_empty() {
        return Err(ApiError::bad_request("缺少设备序列号"));
    }
    state.preview_hub.touch_consumer();
    state
        .preview_hub
        .request_refresh_with_priority([serial.clone()], PreviewPriority::High);
    let hub = state.preview_hub.clone();
    let body_stream = stream! {
        let mut last_version = 0u64;
        loop {
            hub.touch_consumer();
            match hub.get_or_capture(&serial, 360, 48) {
                Ok(frame) => {
                    if frame.version != last_version {
                        last_version = frame.version;
                        let header_chunk = format!(
                            "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n",
                            frame.jpeg.len()
                        );
                        yield Ok::<Bytes, Infallible>(Bytes::from(header_chunk.into_bytes()));
                        yield Ok(Bytes::from(frame.jpeg));
                        yield Ok(Bytes::from_static(b"\r\n"));
                    } else {
                        hub.request_refresh_with_priority([serial.clone()], PreviewPriority::High);
                    }
                }
                Err(_) => {
                    hub.request_refresh_with_priority([serial.clone()], PreviewPriority::High);
                }
            }
            tokio::time::sleep(Duration::from_millis(220)).await;
        }
    };

    let mut response = Response::new(Body::from_stream(body_stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("multipart/x-mixed-replace; boundary=frame"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response.headers_mut().insert(
        header::PRAGMA,
        HeaderValue::from_static("no-cache"),
    );
    Ok(response)
}

fn preview_priority_from_str(value: Option<&str>) -> PreviewPriority {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "high" | "focus" | "selected" => PreviewPriority::High,
        _ => PreviewPriority::Normal,
    }
}

async fn stream_preview_events(
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let hub = state.preview_hub.clone();
    let body_stream = stream! {
        let mut last_versions = HashMap::<String, u64>::new();
        loop {
            hub.touch_consumer();
            let mut changed = Vec::new();
            let mut latest_updated_ms = 0u64;

            for frame in hub.frames() {
                let previous_version = last_versions.get(&frame.serial).copied().unwrap_or(0);
                if frame.version != previous_version {
                    latest_updated_ms = latest_updated_ms.max(frame.updated_at_ms);
                    last_versions.insert(frame.serial.clone(), frame.version);
                    changed.push(PreviewEventFrame {
                        serial: frame.serial,
                        jpeg_base64: base64::engine::general_purpose::STANDARD.encode(frame.jpeg),
                        updated_at_ms: frame.updated_at_ms,
                    });
                }
            }

            if !changed.is_empty() {
                let payload = PreviewEventPayload {
                    frames: changed,
                    updated_at_ms: latest_updated_ms,
                };
                if let Ok(json) = serde_json::to_string(&payload) {
                    let chunk = format!("event: frames\ndata: {json}\n\n");
                    yield Ok::<Bytes, Infallible>(Bytes::from(chunk.into_bytes()));
                }
            }

            tokio::time::sleep(Duration::from_millis(220)).await;
        }
    };

    let mut response = Response::new(Body::from_stream(body_stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response.headers_mut().insert(
        header::CONNECTION,
        HeaderValue::from_static("keep-alive"),
    );
    Ok(response)
}

fn build_action(action: &str, params: &Value) -> Result<BatchAction, ApiError> {
    let action = action.trim();
    match action {
        "keyevent" => Ok(BatchAction::KeyEvent(required_string(params, "key")?)),
        "text" => Ok(BatchAction::Text(required_string(params, "text")?)),
        "clipboard" => Ok(BatchAction::Clipboard(required_string(params, "text")?)),
        "tap" => Ok(BatchAction::Tap {
            x: required_i32(params, "x")?,
            y: required_i32(params, "y")?,
        }),
        "swipe" => Ok(BatchAction::Swipe {
            x1: required_i32(params, "x1")?,
            y1: required_i32(params, "y1")?,
            x2: required_i32(params, "x2")?,
            y2: required_i32(params, "y2")?,
            duration_ms: optional_i32(params, "duration").unwrap_or(300),
        }),
        "gesture_path" => Ok(BatchAction::GesturePath {
            points: required_motion_points(params, "points")?,
        }),
        "launch" => Ok(BatchAction::Launch {
            package: required_string(params, "package")?,
            activity: optional_string(params, "activity").unwrap_or_default(),
        }),
        "stop" => Ok(BatchAction::Stop {
            package: required_string(params, "package")?,
        }),
        "reboot" => Ok(BatchAction::Reboot),
        "reboot_recovery" => Ok(BatchAction::RebootRecovery),
        "reboot_bootloader" => Ok(BatchAction::RebootBootloader),
        "uninstall" => Ok(BatchAction::Uninstall {
            package: required_string(params, "package")?,
        }),
        "clear_data" => Ok(BatchAction::ClearData {
            package: required_string(params, "package")?,
        }),
        "install" => Ok(BatchAction::Install {
            apk_path: PathBuf::from(required_string(params, "apk_path")?),
        }),
        "push_file" => Ok(BatchAction::PushFile {
            local_path: PathBuf::from(required_string(params, "local_path")?),
            remote_path: required_string(params, "remote_path")?,
        }),
        "push_files" => Ok(BatchAction::PushFiles {
            local_paths: required_string_array(params, "local_paths")?
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            remote_dir: required_string(params, "remote_dir")?,
        }),
        "pull_file" => Ok(BatchAction::PullFile {
            remote_path: required_string(params, "remote_path")?,
            local_dir: optional_string(params, "local_dir").map(PathBuf::from),
        }),
        "shell" => Ok(BatchAction::Shell {
            command: required_string(params, "command")?,
        }),
        "screenshot" => Ok(BatchAction::Screenshot),
        "mirror" => Ok(BatchAction::Mirror),
        _ => Err(ApiError::bad_request("不支持的动作")),
    }
}

fn merge_device_profiles(
    mut devices: Vec<adb::DeviceInfo>,
    profiles: &[DeviceProfile],
) -> Vec<adb::DeviceInfo> {
    for device in &mut devices {
        if let Some(profile) = profiles
            .iter()
            .find(|profile| profile.serial == device.serial)
        {
            device.custom_name = profile.custom_name.clone();
            device.custom_number = profile.custom_number.clone();
            device.remark = profile.remark.clone();
            device.tags = profile.tags.clone();
        }
    }
    devices
}

fn required_string(params: &Value, key: &str) -> Result<String, ApiError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ApiError::bad_request(format!("缺少参数: {key}")))
}

fn optional_string(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn required_string_array(params: &Value, key: &str) -> Result<Vec<String>, ApiError> {
    params
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .ok_or_else(|| ApiError::bad_request(format!("缺少参数: {key}")))
}

fn required_motion_points(params: &Value, key: &str) -> Result<Vec<MotionPoint>, ApiError> {
    let items = params
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| ApiError::bad_request(format!("缺少参数: {key}")))?;

    let mut points = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let point = item
            .as_object()
            .ok_or_else(|| ApiError::bad_request(format!("轨迹点格式非法: #{index}")))?;

        let x = parse_i32_value(point.get("x"))
            .ok_or_else(|| ApiError::bad_request(format!("轨迹点缺少或非法 x: #{index}")))?;
        let y = parse_i32_value(point.get("y"))
            .ok_or_else(|| ApiError::bad_request(format!("轨迹点缺少或非法 y: #{index}")))?;

        points.push(MotionPoint {
            x,
            y,
            delay_ms: parse_i32_value(point.get("delay_ms")).unwrap_or_default(),
        });
    }

    Ok(points)
}

fn required_i32(params: &Value, key: &str) -> Result<i32, ApiError> {
    parse_i32_value(params.get(key))
        .ok_or_else(|| ApiError::bad_request(format!("缺少或非法参数: {key}")))
}

fn optional_i32(params: &Value, key: &str) -> Option<i32> {
    parse_i32_value(params.get(key))
}

fn parse_i32_value(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse::<i64>().ok()))
        })
        .and_then(|value| i32::try_from(value).ok())
}

fn persist_run_record(
    base_dir: &std::path::Path,
    task_name: Option<String>,
    action: String,
    serials: Vec<String>,
    results: &[ActionResultPayload],
    template_id: Option<String>,
) {
    let success_count = results.iter().filter(|item| item.ok).count();
    let failure_count = results.len().saturating_sub(success_count);
    let record = TaskRunRecord {
        id: storage::make_id("run"),
        name: task_name.unwrap_or_else(|| action.clone()),
        action,
        created_at: storage::timestamp_label(),
        device_count: serials.len(),
        success_count,
        failure_count,
        serials,
        template_id,
    };
    persist_task_run_record(base_dir, record);
}

fn persist_task_run_record(base_dir: &std::path::Path, record: TaskRunRecord) {
    let _ = storage::append_task_run(base_dir, record);
}

fn current_focus_mirror(state: &Arc<AppState>) -> (bool, Option<String>) {
    let mut mirror = match state.focus_mirror.lock() {
        Ok(mirror) => mirror,
        Err(_) => return (false, None),
    };
    cleanup_focus_mirror(&mut mirror);
    (mirror.child.is_some(), mirror.serial.clone())
}

fn cleanup_focus_mirror(mirror: &mut FocusMirrorState) {
    let exited = mirror
        .child
        .as_mut()
        .and_then(|child| child.try_wait().ok())
        .flatten()
        .is_some();
    if exited {
        mirror.child = None;
        mirror.serial = None;
    }
}

fn stop_focus_mirror_locked(mirror: &mut FocusMirrorState) {
    if let Some(child) = mirror.child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
    mirror.child = None;
    mirror.serial = None;
}

fn serve_local_file(base_dir: PathBuf, file_name: &str) -> Result<Response, ApiError> {
    let safe_name = sanitize_file_name(file_name)?;
    let path = base_dir.join(&safe_name);
    let bytes = std::fs::read(&path).map_err(|err| ApiError::bad_request(err.to_string()))?;
    let mut response = Response::new(bytes.into());
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_file(&safe_name)),
    );
    if safe_name.to_ascii_lowercase().ends_with(".zip") {
        let disposition = format!("attachment; filename=\"{safe_name}\"");
        if let Ok(value) = HeaderValue::from_str(&disposition) {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

fn sanitize_file_name(file_name: &str) -> Result<String, ApiError> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..")
    {
        return Err(ApiError::bad_request("非法文件名"));
    }
    Ok(trimmed.to_string())
}

fn sanitize_upload_name(file_name: &str) -> Result<String, ApiError> {
    let safe_name = sanitize_file_name(file_name)?;
    let lower = safe_name.to_ascii_lowercase();
    if !lower.ends_with(".apk") {
        return Err(ApiError::bad_request("只支持拖入 .apk 文件"));
    }
    Ok(safe_name.replace(' ', "_"))
}

fn run_whep_bridge_command(base_dir: &StdPath, action: &str, serial: &str) -> Result<String, ApiError> {
    let script = whep_bridge_script(base_dir);
    if !script.exists() {
        return Err(ApiError::bad_request(format!(
            "没有找到 WHEP bridge 脚本: {}",
            script.display()
        )));
    }

    let output = Command::new(&script)
        .arg(action)
        .arg(serial)
        .output()
        .map_err(|err| ApiError::bad_request(format!("执行 WHEP bridge 失败: {err}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("WHEP bridge 命令执行失败: {action}")
        };
        return Err(ApiError::bad_request(message));
    }

    Ok(stdout)
}

fn inspect_whep_bridge(base_dir: &StdPath, serial: &str) -> Result<WhepBridgePayload, ApiError> {
    let output = run_whep_bridge_command(base_dir, "status", serial)?;
    Ok(parse_whep_bridge_status(serial, &output))
}

fn parse_whep_bridge_status(serial: &str, output: &str) -> WhepBridgePayload {
    let mut path_key = format!("scrcpy-{}", sanitize_serial_key(serial));
    let mut tmux_running = false;
    let mut supervisor_running = false;
    let mut ffmpeg_running = false;
    let mut scrcpy_running = false;
    let mut whep_url = String::new();
    let mut log_path = String::new();

    for line in output.lines() {
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let key = raw_key.trim();
        let value = raw_value.trim();
        match key {
            "path" => path_key = value.to_string(),
            "tmux" => tmux_running = value.starts_with("running"),
            "super" => supervisor_running = value.starts_with("running"),
            "ffmpeg" => ffmpeg_running = value.starts_with("running"),
            "scrcpy" => scrcpy_running = value.starts_with("running"),
            "WHEP" => whep_url = value.to_string(),
            "LOG" => log_path = value.to_string(),
            _ => {}
        }
    }

    WhepBridgePayload {
        serial: serial.to_string(),
        path_key,
        tmux_running,
        supervisor_running,
        ffmpeg_running,
        scrcpy_running,
        whep_url,
        log_path,
    }
}

fn whep_bridge_script(base_dir: &StdPath) -> PathBuf {
    base_dir.join("scripts").join("scrcpy-whep-bridge.sh")
}

fn sanitize_serial_key(serial: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in serial.trim().chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            result.push(normalized);
            previous_dash = false;
        } else if !previous_dash && !result.is_empty() {
            result.push('-');
            previous_dash = true;
        }
    }
    result.trim_matches('-').to_string()
}

fn infer_material_kind(file_name: &str) -> String {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "mov" | "m4v" | "mkv" => "video",
        "jpg" | "jpeg" | "png" | "webp" => "image",
        "txt" | "md" | "json" => "text",
        "apk" => "apk",
        _ => "file",
    }
    .to_string()
}

fn system_time_millis(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn build_apk_metadata_cache_entry(
    path: &std::path::Path,
    size_bytes: u64,
    inspect: &ApkInspectInfo,
) -> ApkMetadataCacheEntry {
    let modified_at_ms = std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(system_time_millis)
        .unwrap_or_default();
    ApkMetadataCacheEntry {
        artifact_path: path.display().to_string(),
        size_bytes,
        modified_at_ms,
        package_name: inspect.package_name.clone(),
        version_name: inspect.version_name.clone(),
        version_code: inspect.version_code.clone(),
        inspect_error: inspect.inspect_error.clone(),
    }
}

fn inspect_info_from_cache(entry: &ApkMetadataCacheEntry) -> ApkInspectInfo {
    ApkInspectInfo {
        package_name: entry.package_name.clone(),
        version_name: entry.version_name.clone(),
        version_code: entry.version_code.clone(),
        inspect_error: entry.inspect_error.clone(),
    }
}

fn resolve_apk_metadata(
    _base_dir: &std::path::Path,
    path: &std::path::Path,
    size_bytes: u64,
    modified_at_ms: u64,
    cache: &mut Vec<ApkMetadataCacheEntry>,
    cache_changed: &mut bool,
) -> ApkInspectInfo {
    let artifact_path = path.display().to_string();
    if let Some(entry) = cache.iter().find(|entry| {
        entry.artifact_path == artifact_path
            && entry.size_bytes == size_bytes
            && entry.modified_at_ms == modified_at_ms
    }) {
        return inspect_info_from_cache(entry);
    }
    let inspect = inspect_apk_metadata(path);
    cache.retain(|entry| entry.artifact_path != artifact_path);
    cache.push(ApkMetadataCacheEntry {
        artifact_path,
        size_bytes,
        modified_at_ms,
        package_name: inspect.package_name.clone(),
        version_name: inspect.version_name.clone(),
        version_code: inspect.version_code.clone(),
        inspect_error: inspect.inspect_error.clone(),
    });
    *cache_changed = true;
    inspect
}

fn upsert_apk_metadata_cache_entry(
    base_dir: &std::path::Path,
    entry: ApkMetadataCacheEntry,
) -> Result<(), String> {
    let mut cache = storage::load_apk_metadata_cache(base_dir);
    cache.retain(|current| current.artifact_path != entry.artifact_path);
    cache.push(entry);
    storage::save_apk_metadata_cache(base_dir, &cache)
}

fn inspect_apk_metadata(path: &std::path::Path) -> ApkInspectInfo {
    let analyzer = find_apkanalyzer_binary();
    if analyzer.is_none() {
        return ApkInspectInfo {
            inspect_error: Some("当前环境没有 apkanalyzer，暂时无法识别包名和版本".to_string()),
            ..Default::default()
        };
    }
    if !java_runtime_available() {
        return ApkInspectInfo {
            inspect_error: Some("当前 Mac 没有安装 Java 运行时，暂时无法自动识别 APK 包名和版本".to_string()),
            ..Default::default()
        };
    }

    let analyzer = analyzer.unwrap_or_else(|| PathBuf::from("apkanalyzer"));
    let package_name = run_apkanalyzer_field(&analyzer, "application-id", path);
    let version_name = run_apkanalyzer_field(&analyzer, "version-name", path);
    let version_code = run_apkanalyzer_field(&analyzer, "version-code", path);

    let package_value = package_name.ok().filter(|value| !value.is_empty());
    let version_name_value = version_name.ok().filter(|value| !value.is_empty());
    let version_code_value = version_code.ok().filter(|value| !value.is_empty());

    let inspect_error =
        if package_value.is_none() && version_name_value.is_none() && version_code_value.is_none() {
            Some("暂时没能从 APK 里识别出包名或版本信息".to_string())
        } else {
            None
        };

    ApkInspectInfo {
        package_name: package_value,
        version_name: version_name_value,
        version_code: version_code_value,
        inspect_error,
    }
}

fn find_apkanalyzer_binary() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/apkanalyzer",
        "/usr/local/bin/apkanalyzer",
        "apkanalyzer",
    ];
    candidates.iter().find_map(|candidate| {
        let path = PathBuf::from(candidate);
        if candidate.contains('/') {
            path.exists().then_some(path)
        } else {
            Command::new("sh")
                .arg("-lc")
                .arg(format!("command -v {candidate}"))
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| {
                    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    (!value.is_empty()).then(|| PathBuf::from(value))
                })
        }
    })
}

fn java_runtime_available() -> bool {
    Command::new("java")
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_apkanalyzer_field(
    analyzer: &std::path::Path,
    field: &str,
    apk_path: &std::path::Path,
) -> Result<String, String> {
    let output = Command::new(analyzer)
        .args(["manifest", field])
        .arg(apk_path)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "读取 APK 信息失败".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn content_type_for_file(file_name: &str) -> &'static str {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// Host / VM request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SaveHostRequest {
    name: String,
    host: String,
    #[serde(default = "default_ssh_port")]
    ssh_port: u16,
    #[serde(default = "default_user")]
    user: String,
    password: String,
    #[serde(default = "default_port_start")]
    adb_port_start: u16,
    #[serde(default = "default_port_end")]
    adb_port_end: u16,
    #[serde(default = "default_image")]
    image: String,
}
fn default_ssh_port() -> u16 { 22 }
fn default_user() -> String { "root".to_string() }
fn default_port_start() -> u16 { 5550 }
fn default_port_end() -> u16 { 5599 }
fn default_image() -> String { "cnflysky/redroid-rk3588:lineage-20".to_string() }

#[derive(Deserialize)]
struct CreateVmsRequest { count: usize }

#[derive(Deserialize)]
struct BatchVmRequest { names: Vec<String>, action: String }

// ---------------------------------------------------------------------------
// Host / VM handlers
// ---------------------------------------------------------------------------

async fn get_hosts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        Ok(Json(serde_json::json!({ "hosts": hosts })))
    })
    .await
}

async fn save_host(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveHostRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut hosts = host::load_hosts(&base_dir);
        let new_host = HostConfig {
            id: storage::make_id("host"),
            name: payload.name,
            host: payload.host,
            ssh_port: payload.ssh_port,
            user: payload.user,
            password: payload.password,
            adb_port_start: payload.adb_port_start,
            adb_port_end: payload.adb_port_end,
            image: payload.image,
        };
        let id = new_host.id.clone();
        hosts.push(new_host);
        host::save_hosts(&base_dir, &hosts).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "id": id, "hosts": hosts })))
    })
    .await
}

async fn update_host(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<SaveHostRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut hosts = host::load_hosts(&base_dir);
        let entry = hosts
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?;
        entry.name = payload.name;
        entry.host = payload.host;
        entry.ssh_port = payload.ssh_port;
        entry.user = payload.user;
        entry.password = payload.password;
        entry.adb_port_start = payload.adb_port_start;
        entry.adb_port_end = payload.adb_port_end;
        entry.image = payload.image;
        host::save_hosts(&base_dir, &hosts).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "hosts": hosts })))
    })
    .await
}

async fn delete_host_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut hosts = host::load_hosts(&base_dir);
        let before = hosts.len();
        hosts.retain(|h| h.id != id);
        if hosts.len() == before {
            return Err(ApiError::bad_request(format!("未找到宿主机: {id}")));
        }
        host::save_hosts(&base_dir, &hosts).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "hosts": hosts })))
    })
    .await
}

async fn test_host_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let output = host::test_connection(&config).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "output": output })))
    })
    .await
}

async fn get_vms(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let vms = host::list_vms(&config).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "vms": vms })))
    })
    .await
}

async fn create_vms_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<CreateVmsRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    let auto_connector = Arc::clone(&state.auto_connector);
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let created = host::create_vms(&config, payload.count).map_err(ApiError::bad_request)?;
        // Auto-connect newly created VMs
        let vms = host::list_vms(&config).unwrap_or_default();
        for name in &created {
            if let Some(vm) = vms.iter().find(|v| &v.name == name) {
                if vm.host_port > 0 {
                    auto_connector.trigger_connect(&config.host, vm.host_port);
                }
            }
        }
        Ok(Json(serde_json::json!({ "created": created })))
    })
    .await
}

async fn start_vm(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let output = host::vm_action(&config, &name, "start").map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "output": output })))
    })
    .await
}

async fn stop_vm(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let output = host::vm_action(&config, &name, "stop").map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "output": output })))
    })
    .await
}

async fn restart_vm(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let output = host::vm_action(&config, &name, "restart").map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "output": output })))
    })
    .await
}

async fn delete_vm_handler(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let output = host::delete_vm(&config, &name, true).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "output": output })))
    })
    .await
}

async fn batch_vm_action_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<BatchVmRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let config = hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request(format!("未找到宿主机: {id}")))?
            .clone();
        let results = host::batch_vm_action(&config, &payload.names, &payload.action);
        let items: Vec<Value> = results
            .into_iter()
            .map(|(name, result)| match result {
                Ok(output) => serde_json::json!({ "name": name, "ok": true, "output": output }),
                Err(err) => serde_json::json!({ "name": name, "ok": false, "error": err }),
            })
            .collect();
        Ok(Json(serde_json::json!({ "results": items })))
    })
    .await
}

// ---------------------------------------------------------------------------
// Rotate (one-click new device) + Provision + Profile management
// ---------------------------------------------------------------------------

async fn rotate_vms_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let hosts = host::load_hosts(&state.base_dir);
    let config = hosts.iter().find(|h| h.id == id)
        .ok_or_else(|| ApiError::bad_request(format!("Host {id} not found")))?;
    let count = body.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as usize;

    // Rotate: delete all VMs, recreate with fresh profiles
    let existing = host::list_vms(config).unwrap_or_default();
    for vm in &existing {
        let _ = host::delete_vm(config, &vm.name, true);
    }

    let created = host::create_vms_v2(config, count).map_err(ApiError::bad_request)?;

    // Provision each (async in background)
    for name in &created {
        let _ = host::wait_and_provision(config, name, 120);
    }

    Ok(Json(serde_json::json!({
        "rotated": created.len(),
        "containers": created,
    })))
}

async fn provision_vm_handler(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let hosts = host::load_hosts(&state.base_dir);
    let config = hosts.iter().find(|h| h.id == id)
        .ok_or_else(|| ApiError::bad_request(format!("Host {id} not found")))?;

    let result = host::inject_anti_detection_props(config, &name).map_err(ApiError::bad_request)?;
    Ok(Json(serde_json::json!({ "status": "provisioned", "output": result })))
}

async fn profile_stats_handler() -> Json<Value> {
    let count = |dir: &str| -> usize {
        std::fs::read_dir(format!("/root/profile_pool/{dir}"))
            .map(|d| d.count())
            .unwrap_or(0)
    };
    Json(serde_json::json!({
        "available": count("available"),
        "active": count("active"),
        "burned": count("burned"),
    }))
}

async fn generate_profiles_handler(
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let count = body.get("count").and_then(|v| v.as_u64()).unwrap_or(10);
    let output = std::process::Command::new("python3")
        .args(["/root/profile_pool/generator.py", &count.to_string()])
        .output()
        .map_err(|e| ApiError::bad_request(format!("Generator failed: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Json(serde_json::json!({
        "generated": count,
        "output": stdout.trim(),
    })))
}

async fn get_host_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let stats = state
        .host_monitor
        .get_stats(&id)
        .unwrap_or_else(|| HostStats {
            host_id: id,
            ..Default::default()
        });
    Ok(Json(serde_json::json!({ "stats": stats })))
}

async fn stream_host_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let monitor = Arc::clone(&state.host_monitor);
    let body_stream = stream! {
        loop {
            let stats = monitor
                .get_stats(&id)
                .unwrap_or_else(|| HostStats {
                    host_id: id.clone(),
                    ..Default::default()
                });
            if let Ok(json) = serde_json::to_string(&stats) {
                let chunk = format!("event: stats\ndata: {json}\n\n");
                yield Ok::<Bytes, Infallible>(Bytes::from(chunk.into_bytes()));
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    let mut response = Response::new(Body::from_stream(body_stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response.headers_mut().insert(
        header::CONNECTION,
        HeaderValue::from_static("keep-alive"),
    );
    Ok(response)
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

async fn run_blocking<T, F>(job: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
{
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|err| ApiError::bad_request(format!("后台任务执行失败: {err}")))?
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "error": self.message,
            })),
        )
            .into_response()
    }
}

// ── WebSocket interactive control ──────────────────────────────────────

async fn ws_interactive(
    Path(serial): Path<String>,
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let serial = serial.trim().to_string();
    ws.on_upgrade(move |socket| handle_interactive(socket, serial, state))
}

async fn ws_interactive_sync(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_interactive_sync(socket, state))
}

async fn handle_interactive(mut socket: WebSocket, serial: String, state: Arc<AppState>) {
    // Request high-priority preview for this device
    state.preview_hub.request_refresh_with_priority([serial.clone()], PreviewPriority::High);

    while let Some(Ok(msg)) = socket.recv().await {
        let Message::Text(text) = msg else { continue };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else { continue };

        let event_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
        let result = match event_type {
            "tap" => {
                let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                let (hx, hy) = bezier::humanize_tap(x, y);
                let serial = serial.clone();
                tokio::task::spawn_blocking(move || {
                    adb::fast_shell_command(&serial, &format!("input tap {hx} {hy}"))
                }).await.ok().and_then(|r| r.ok())
            }
            "swipe" => {
                let x1 = payload.get("x1").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y1 = payload.get("y1").and_then(Value::as_i64).unwrap_or(0) as i32;
                let x2 = payload.get("x2").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y2 = payload.get("y2").and_then(Value::as_i64).unwrap_or(0) as i32;
                let dur = payload.get("duration").and_then(Value::as_i64).unwrap_or(300) as u32;
                let points = bezier::humanize_swipe(x1, y1, x2, y2, dur);
                let cmd = bezier::build_swipe_shell_script(&points);
                let serial = serial.clone();
                tokio::task::spawn_blocking(move || {
                    adb::fast_shell_command(&serial, &cmd)
                }).await.ok().and_then(|r| r.ok())
            }
            "longpress" => {
                let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                let serial = serial.clone();
                tokio::task::spawn_blocking(move || {
                    adb::fast_shell_command(&serial, &format!("input swipe {x} {y} {x} {y} 600"))
                }).await.ok().and_then(|r| r.ok())
            }
            "keyevent" => {
                let key = payload.get("key").and_then(Value::as_str).unwrap_or("HOME").to_string();
                let normalized = if key.starts_with("KEYCODE_") || key.chars().all(|c| c.is_ascii_digit()) {
                    key
                } else {
                    format!("KEYCODE_{}", key.to_uppercase())
                };
                let serial = serial.clone();
                tokio::task::spawn_blocking(move || {
                    adb::fast_shell_command(&serial, &format!("input keyevent {normalized}"))
                }).await.ok().and_then(|r| r.ok())
            }
            "text" => {
                let text = payload.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                let serial = serial.clone();
                tokio::task::spawn_blocking(move || {
                    adb::fast_text_via_clipboard(&serial, &text)
                        .map(|o| o.stdout)
                        .or_else(|_| {
                            let encoded = text.replace(' ', "%s");
                            adb::fast_shell_command(&serial, &format!("input text {encoded}"))
                        })
                }).await.ok().and_then(|r| r.ok())
            }
            _ => None,
        };

        // Send result back
        let ok = result.is_some();
        let reply = serde_json::json!({ "type": "result", "ok": ok });
        if socket.send(Message::Text(reply.to_string().into())).await.is_err() {
            break;
        }

        // Request preview refresh after action
        state.preview_hub.request_refresh_with_priority([serial.clone()], PreviewPriority::High);
    }
}

async fn handle_interactive_sync(mut socket: WebSocket, state: Arc<AppState>) {
    while let Some(Ok(msg)) = socket.recv().await {
        let Message::Text(text) = msg else { continue };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else { continue };

        let serials: Vec<String> = payload.get("serials")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).map(String::from).collect())
            .unwrap_or_default();

        if serials.is_empty() { continue; }

        let event_type = payload.get("type").and_then(Value::as_str).unwrap_or("").to_string();

        // Build the shell command once
        let cmd = match event_type.as_str() {
            "tap" => {
                let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                let (hx, hy) = bezier::humanize_tap(x, y);
                Some(format!("input tap {hx} {hy}"))
            }
            "swipe" => {
                let x1 = payload.get("x1").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y1 = payload.get("y1").and_then(Value::as_i64).unwrap_or(0) as i32;
                let x2 = payload.get("x2").and_then(Value::as_i64).unwrap_or(0) as i32;
                let y2 = payload.get("y2").and_then(Value::as_i64).unwrap_or(0) as i32;
                let dur = payload.get("duration").and_then(Value::as_i64).unwrap_or(300) as u32;
                let points = bezier::humanize_swipe(x1, y1, x2, y2, dur);
                Some(bezier::build_swipe_shell_script(&points))
            }
            "longpress" => {
                let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0);
                let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0);
                Some(format!("input swipe {x} {y} {x} {y} 600"))
            }
            "keyevent" => {
                let key = payload.get("key").and_then(Value::as_str).unwrap_or("HOME");
                let normalized = if key.starts_with("KEYCODE_") || key.chars().all(|c| c.is_ascii_digit()) {
                    key.to_string()
                } else {
                    format!("KEYCODE_{}", key.to_uppercase())
                };
                Some(format!("input keyevent {normalized}"))
            }
            _ => None,
        };

        let Some(cmd) = cmd else { continue };

        // Broadcast to all serials in parallel
        let mut handles = Vec::new();
        for serial in &serials {
            let serial = serial.clone();
            let cmd = cmd.clone();
            handles.push(tokio::task::spawn_blocking(move || {
                adb::fast_shell_command(&serial, &cmd)
            }));
        }
        let mut ok_count = 0usize;
        for handle in handles {
            if let Ok(Ok(_)) = handle.await {
                ok_count += 1;
            }
        }

        // Refresh previews for all touched devices
        state.preview_hub.request_refresh_with_priority(serials.clone(), PreviewPriority::High);

        let reply = serde_json::json!({
            "type": "result",
            "ok": ok_count > 0,
            "success_count": ok_count,
            "total": serials.len()
        });
        if socket.send(Message::Text(reply.to_string().into())).await.is_err() {
            break;
        }
    }
}

async fn ws_scrcpy(
    Path(serial): Path<String>,
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let serial = serial.trim().to_string();
    ws.on_upgrade(move |socket| handle_scrcpy_session(socket, serial, state))
}

async fn handle_scrcpy_session(socket: WebSocket, serial: String, state: Arc<AppState>) {
    // Start scrcpy session for this device
    let session = match state.scrcpy.start_session(&serial) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[scrcpy-ws] start failed {serial}: {e}");
            return;
        }
    };

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (frame_tx, mut frame_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);

    // Video reader thread: blocking TCP read -> tokio channel
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
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("[scrcpy-ws] video read error {video_serial}: {e}");
                    break;
                }
            }
        }
    });

    // Forward video frames to WebSocket as binary
    let send_task = tokio::spawn(async move {
        while let Some(data) = frame_rx.recv().await {
            if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive control events from WebSocket
    let control_session = session.clone();
    let control_serial = serial.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            let text = match msg {
                Message::Text(t) => t.to_string(),
                _ => continue,
            };
            let payload: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };
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
                        let (w, h) = (s.width, s.height);
                        s.inject_touch(0, x, y, w, h)
                    }
                    "touch_move" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let (w, h) = (s.width, s.height);
                        s.inject_touch(2, x, y, w, h)
                    }
                    "touch_up" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let (w, h) = (s.width, s.height);
                        s.inject_touch(1, x, y, w, h)
                    }
                    "keyevent" => {
                        let keycode = payload.get("keycode").and_then(Value::as_u64).unwrap_or(0) as u32;
                        s.key_press(keycode)
                    }
                    "text" => {
                        let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
                        s.inject_text(text)
                    }
                    "scroll" => {
                        let x = payload.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y = payload.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let sx = payload.get("scrollX").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let sy = payload.get("scrollY").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let (w, h) = (s.width, s.height);
                        s.inject_scroll(x, y, w, h, sx, sy)
                    }
                    "clipboard" => {
                        let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
                        let paste = payload.get("paste").and_then(Value::as_bool).unwrap_or(true);
                        s.set_clipboard(text, paste)
                    }
                    "swipe" => {
                        let x1 = payload.get("x1").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y1 = payload.get("y1").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let x2 = payload.get("x2").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let y2 = payload.get("y2").and_then(Value::as_i64).unwrap_or(0) as i32;
                        let steps = payload.get("steps").and_then(Value::as_u64).unwrap_or(20) as u32;
                        s.swipe(x1, y1, x2, y2, steps)
                    }
                    "back" => s.press_back(),
                    "home" => s.press_home(),
                    "recents" => s.press_recents(),
                    "rotate" => s.rotate_device(),
                    "screen_power" => {
                        let mode = payload.get("mode").and_then(Value::as_u64).unwrap_or(2) as u8;
                        s.set_screen_power_mode(mode)
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

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    state.scrcpy.stop_session(&serial);
}

// ── FPS endpoint ───────────────────────────────────────────────────────

async fn get_scrcpy_fps(
    Path(serial): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let serial = serial.trim().to_string();
    if let Some(session) = state.scrcpy.get_session(&serial) {
        if let Ok(s) = session.lock() {
            let fps = s.fps_counter.fps();
            return Json(serde_json::json!({
                "serial": serial,
                "fps": fps,
                "width": s.width,
                "height": s.height,
                "codec_id": s.codec_id,
            }))
            .into_response();
        }
    }
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "no active session"}))).into_response()
}

// ── Group control: fan-out single input to N devices ───────────────────

#[derive(Deserialize)]
struct GroupControlRequest {
    serials: Vec<String>,
    event: GroupControlEvent,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum GroupControlEvent {
    #[serde(rename = "tap")]
    Tap { x: i32, y: i32 },
    #[serde(rename = "touch")]
    Touch { action: u8, x: i32, y: i32 },
    #[serde(rename = "swipe")]
    Swipe { x1: i32, y1: i32, x2: i32, y2: i32, #[serde(default = "default_steps")] steps: u32 },
    #[serde(rename = "keyevent")]
    KeyPress { keycode: u32 },
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "scroll")]
    Scroll { x: i32, y: i32, #[serde(rename = "scrollX")] scroll_x: i32, #[serde(rename = "scrollY")] scroll_y: i32 },
    #[serde(rename = "clipboard")]
    SetClipboard { text: String, #[serde(default = "default_true")] paste: bool },
    #[serde(rename = "back")]
    Back,
    #[serde(rename = "home")]
    Home,
    #[serde(rename = "recents")]
    Recents,
    #[serde(rename = "rotate")]
    RotateDevice,
    #[serde(rename = "screen_power")]
    SetScreenPower { #[serde(default = "default_screen_normal")] mode: u8 },
}

fn default_steps() -> u32 { 20 }
fn default_true() -> bool { true }
fn default_screen_normal() -> u8 { 2 }

impl From<&GroupControlEvent> for ControlEvent {
    fn from(e: &GroupControlEvent) -> Self {
        match e {
            GroupControlEvent::Tap { x, y } => ControlEvent::Tap { x: *x, y: *y },
            GroupControlEvent::Touch { action, x, y } => ControlEvent::Touch { action: *action, x: *x, y: *y },
            GroupControlEvent::Swipe { x1, y1, x2, y2, steps } => ControlEvent::Swipe { x1: *x1, y1: *y1, x2: *x2, y2: *y2, steps: *steps },
            GroupControlEvent::KeyPress { keycode } => ControlEvent::KeyPress { keycode: *keycode },
            GroupControlEvent::Text { text } => ControlEvent::Text { text: text.clone() },
            GroupControlEvent::Scroll { x, y, scroll_x, scroll_y } => ControlEvent::Scroll { x: *x, y: *y, scroll_x: *scroll_x, scroll_y: *scroll_y },
            GroupControlEvent::SetClipboard { text, paste } => ControlEvent::SetClipboard { text: text.clone(), paste: *paste },
            GroupControlEvent::Back => ControlEvent::Back,
            GroupControlEvent::Home => ControlEvent::Home,
            GroupControlEvent::Recents => ControlEvent::Recents,
            GroupControlEvent::RotateDevice => ControlEvent::RotateDevice,
            GroupControlEvent::SetScreenPower { mode } => ControlEvent::SetScreenPower { mode: *mode },
        }
    }
}

async fn group_control(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GroupControlRequest>,
) -> impl IntoResponse {
    let event: ControlEvent = (&req.event).into();

    // Run fan-out in blocking context (uses scoped threads internally)
    let serials = req.serials.clone();
    let results = tokio::task::spawn_blocking({
        // We need to pass the manager reference safely.
        // Since ScrcpyManager is behind Arc<AppState>, clone the Arc.
        let state = state.clone();
        move || GroupController::dispatch(&state.scrcpy, &serials, &event)
    })
    .await
    .unwrap_or_default();

    let result_map: HashMap<String, String> = results
        .into_iter()
        .map(|(serial, r)| {
            (
                serial,
                match r {
                    Ok(()) => "ok".to_string(),
                    Err(e) => e,
                },
            )
        })
        .collect();

    Json(serde_json::json!({
        "dispatched": req.serials.len(),
        "results": result_map,
    }))
}
