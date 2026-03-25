use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceGroup {
    pub name: String,
    pub serials: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub id: String,
    pub name: String,
    pub action: String,
    #[serde(default)]
    pub params: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskRunRecord {
    pub id: String,
    pub name: String,
    pub action: String,
    pub created_at: String,
    pub device_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    #[serde(default)]
    pub serials: Vec<String>,
    pub template_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecordingStep {
    pub action: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub delay_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecordingTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub steps: Vec<RecordingStep>,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub serial: String,
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
pub struct QuickPhrase {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TextTemplatePreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub template: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdbCommandPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub command: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MaterialAsset {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub artifact_path: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountBinding {
    pub id: String,
    #[serde(default)]
    pub platform: String,
    pub account_name: String,
    #[serde(default)]
    pub account_group: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub note: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub ref_id: String,
    #[serde(default)]
    pub serials: Vec<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub note: String,
    pub created_at: String,
    #[serde(default)]
    pub last_run_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApkMetadataCacheEntry {
    pub artifact_path: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub modified_at_ms: u64,
    pub package_name: Option<String>,
    pub version_name: Option<String>,
    pub version_code: Option<String>,
    pub inspect_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

pub fn groups_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("groups.json")
}

pub fn templates_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("task_templates.json")
}

pub fn task_runs_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("task_runs.json")
}

pub fn recordings_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("recordings.json")
}

pub fn device_profiles_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("device_profiles.json")
}

pub fn quick_phrases_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("quick_phrases.json")
}

pub fn text_templates_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("text_templates.json")
}

pub fn adb_presets_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("adb_presets.json")
}

pub fn materials_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("materials.json")
}

pub fn accounts_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("accounts.json")
}

pub fn queue_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("task_queue.json")
}

pub fn apk_metadata_cache_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("apk_metadata_cache.json")
}

pub fn screenshot_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("screenshots")
}

pub fn export_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("exports")
}

pub fn upload_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("uploads")
}

pub fn material_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("materials")
}

// ---------------------------------------------------------------------------
// Atomic write & generic JSON helpers
// ---------------------------------------------------------------------------

pub fn atomic_write(target: &Path, data: &[u8]) -> Result<(), String> {
    let dir = target.parent().ok_or_else(|| "无效路径".to_string())?;
    let tmp_path = dir.join(format!(".tmp-{}", timestamp_millis()));
    std::fs::write(&tmp_path, data).map_err(|err| format!("写入临时文件失败: {err}"))?;
    std::fs::rename(&tmp_path, target).map_err(|err| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("重命名失败: {err}")
    })
}

fn load_json_list<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|_| "[]".to_string());
    match serde_json::from_str::<Vec<T>>(&raw) {
        Ok(items) => items,
        Err(err) => {
            eprintln!("[storage] 解析 {} 失败: {err}，返回空列表", path.display());
            Vec::new()
        }
    }
}

fn save_json_list<T: Serialize>(path: &Path, items: &[T]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(items).map_err(|err| err.to_string())?;
    atomic_write(path, format!("{json}\n").as_bytes())
}

// ---------------------------------------------------------------------------
// Layout initialisation
// ---------------------------------------------------------------------------

pub fn ensure_layout(base_dir: &Path) -> Result<(), String> {
    let data_dir = base_dir.join("data");
    for dir in [
        &data_dir,
        &data_dir.join("screenshots"),
        &data_dir.join("exports"),
        &data_dir.join("uploads"),
        &data_dir.join("materials"),
    ] {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    }

    let json_files = [
        groups_path(base_dir),
        templates_path(base_dir),
        task_runs_path(base_dir),
        recordings_path(base_dir),
        device_profiles_path(base_dir),
        quick_phrases_path(base_dir),
        text_templates_path(base_dir),
        adb_presets_path(base_dir),
        materials_path(base_dir),
        accounts_path(base_dir),
        queue_path(base_dir),
        apk_metadata_cache_path(base_dir),
    ];

    for path in json_files {
        if !path.exists() {
            std::fs::write(&path, "[]\n").map_err(|err| err.to_string())?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

pub fn format_system_time(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    timestamp_label_from_seconds(seconds)
}

// ---------------------------------------------------------------------------
// Public load / save thin wrappers
// ---------------------------------------------------------------------------

pub fn load_groups(base_dir: &Path) -> Vec<DeviceGroup> {
    load_json_list(&groups_path(base_dir))
}

pub fn save_groups(base_dir: &Path, groups: &[DeviceGroup]) -> Result<(), String> {
    save_json_list(&groups_path(base_dir), groups)
}

pub fn load_task_templates(base_dir: &Path) -> Vec<TaskTemplate> {
    load_json_list(&templates_path(base_dir))
}

pub fn save_task_templates(base_dir: &Path, templates: &[TaskTemplate]) -> Result<(), String> {
    save_json_list(&templates_path(base_dir), templates)
}

pub fn load_task_runs(base_dir: &Path) -> Vec<TaskRunRecord> {
    load_json_list(&task_runs_path(base_dir))
}

pub fn append_task_run(
    base_dir: &Path,
    record: TaskRunRecord,
) -> Result<Vec<TaskRunRecord>, String> {
    let mut runs = load_task_runs(base_dir);
    runs.insert(0, record);
    if runs.len() > 120 {
        runs.truncate(120);
    }
    save_json_list(&task_runs_path(base_dir), &runs)?;
    Ok(runs)
}

pub fn load_recordings(base_dir: &Path) -> Vec<RecordingTemplate> {
    load_json_list(&recordings_path(base_dir))
}

pub fn save_recordings(base_dir: &Path, recordings: &[RecordingTemplate]) -> Result<(), String> {
    save_json_list(&recordings_path(base_dir), recordings)
}

pub fn load_device_profiles(base_dir: &Path) -> Vec<DeviceProfile> {
    load_json_list(&device_profiles_path(base_dir))
}

pub fn save_device_profiles(base_dir: &Path, profiles: &[DeviceProfile]) -> Result<(), String> {
    save_json_list(&device_profiles_path(base_dir), profiles)
}

pub fn load_quick_phrases(base_dir: &Path) -> Vec<QuickPhrase> {
    load_json_list(&quick_phrases_path(base_dir))
}

pub fn save_quick_phrases(base_dir: &Path, phrases: &[QuickPhrase]) -> Result<(), String> {
    save_json_list(&quick_phrases_path(base_dir), phrases)
}

pub fn load_text_templates(base_dir: &Path) -> Vec<TextTemplatePreset> {
    load_json_list(&text_templates_path(base_dir))
}

pub fn save_text_templates(base_dir: &Path, templates: &[TextTemplatePreset]) -> Result<(), String> {
    save_json_list(&text_templates_path(base_dir), templates)
}

pub fn load_adb_presets(base_dir: &Path) -> Vec<AdbCommandPreset> {
    load_json_list(&adb_presets_path(base_dir))
}

pub fn save_adb_presets(base_dir: &Path, presets: &[AdbCommandPreset]) -> Result<(), String> {
    save_json_list(&adb_presets_path(base_dir), presets)
}

pub fn load_materials(base_dir: &Path) -> Vec<MaterialAsset> {
    load_json_list(&materials_path(base_dir))
}

pub fn save_materials(base_dir: &Path, items: &[MaterialAsset]) -> Result<(), String> {
    save_json_list(&materials_path(base_dir), items)
}

pub fn load_accounts(base_dir: &Path) -> Vec<AccountBinding> {
    load_json_list(&accounts_path(base_dir))
}

pub fn save_accounts(base_dir: &Path, items: &[AccountBinding]) -> Result<(), String> {
    save_json_list(&accounts_path(base_dir), items)
}

pub fn load_queue(base_dir: &Path) -> Vec<QueueItem> {
    load_json_list(&queue_path(base_dir))
}

pub fn save_queue(base_dir: &Path, items: &[QueueItem]) -> Result<(), String> {
    save_json_list(&queue_path(base_dir), items)
}

pub fn load_apk_metadata_cache(base_dir: &Path) -> Vec<ApkMetadataCacheEntry> {
    load_json_list(&apk_metadata_cache_path(base_dir))
}

pub fn save_apk_metadata_cache(
    base_dir: &Path,
    items: &[ApkMetadataCacheEntry],
) -> Result<(), String> {
    save_json_list(&apk_metadata_cache_path(base_dir), items)
}

// ---------------------------------------------------------------------------
// ID & timestamp helpers
// ---------------------------------------------------------------------------

pub fn make_id(prefix: &str) -> String {
    let ts = timestamp_millis();
    let rand: u32 = (ts as u32) ^ (std::process::id() << 16) ^ (ts >> 32) as u32;
    format!("{prefix}-{ts}-{rand:08x}")
}

pub fn timestamp_label() -> String {
    timestamp_millis().to_string()
}

fn timestamp_label_from_seconds(seconds: u64) -> String {
    seconds.to_string()
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // -----------------------------------------------------------------------
    // make_id
    // -----------------------------------------------------------------------

    #[test]
    fn make_id_format() {
        let id = make_id("grp");
        assert!(id.starts_with("grp-"), "id should start with prefix: {id}");
        assert!(id.contains('-'), "id should contain dashes: {id}");
        // The hex suffix is the part after the last dash
        let last_part = id.rsplit('-').next().unwrap();
        assert!(
            last_part.chars().all(|c| c.is_ascii_hexdigit()),
            "last segment should be hex: {last_part}"
        );
    }

    #[test]
    fn make_id_uniqueness() {
        // make_id uses millis + pid, so we add a small sleep to ensure
        // the timestamp component differs between calls.
        let id1 = make_id("tpl");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = make_id("tpl");
        assert_ne!(id1, id2, "calls separated by 2ms should produce different IDs");
    }

    // -----------------------------------------------------------------------
    // timestamp_label_from_seconds
    // -----------------------------------------------------------------------

    #[test]
    fn timestamp_label_from_seconds_converts() {
        let label = timestamp_label_from_seconds(1_700_000_000);
        assert_eq!(label, "1700000000");
    }

    // -----------------------------------------------------------------------
    // ensure_layout + load/save round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn ensure_layout_and_round_trip() {
        let base = std::env::temp_dir().join(format!("agc_test_{}", std::process::id()));
        // Clean up from any previous run
        let _ = fs::remove_dir_all(&base);

        ensure_layout(&base).expect("ensure_layout should succeed");

        // Verify directories were created
        assert!(base.join("data").is_dir());
        assert!(base.join("data").join("screenshots").is_dir());
        assert!(base.join("data").join("exports").is_dir());
        assert!(base.join("data").join("uploads").is_dir());
        assert!(base.join("data").join("materials").is_dir());

        // Verify JSON files were created
        assert!(groups_path(&base).exists());
        assert!(templates_path(&base).exists());

        // Round-trip: save then load groups
        let groups = vec![
            DeviceGroup {
                name: "Group A".to_string(),
                serials: vec!["serial-1".to_string(), "serial-2".to_string()],
            },
            DeviceGroup {
                name: "Group B".to_string(),
                serials: vec!["serial-3".to_string()],
            },
        ];

        save_groups(&base, &groups).expect("save_groups should succeed");
        let loaded = load_groups(&base);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Group A");
        assert_eq!(loaded[0].serials.len(), 2);
        assert_eq!(loaded[1].name, "Group B");
        assert_eq!(loaded[1].serials[0], "serial-3");

        // Clean up
        let _ = fs::remove_dir_all(&base);
    }

    // -----------------------------------------------------------------------
    // atomic_write reliability
    // -----------------------------------------------------------------------

    #[test]
    fn atomic_write_reliability() {
        let base = std::env::temp_dir().join(format!("agc_atomic_{}", std::process::id()));
        let _ = fs::create_dir_all(&base);
        let target = base.join("test_file.txt");

        let content = b"hello, atomic write!";
        atomic_write(&target, content).expect("atomic_write should succeed");

        let read_back = fs::read_to_string(&target).expect("should read file back");
        assert_eq!(read_back, "hello, atomic write!");

        // Clean up
        let _ = fs::remove_dir_all(&base);
    }
}
