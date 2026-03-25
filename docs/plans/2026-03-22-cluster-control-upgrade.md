# 群控主板集群管控升级 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the group control app from an ADB command sender to a full cloud-phone cluster control platform with host management, container lifecycle, auto-connect, monitoring, interactive control, and recording.

**Architecture:** Rust Web API (`src/bin/web.rs`) gains new modules (`host.rs`, `auto_connector.rs`, `host_monitor.rs`) exposed as HTTP/WebSocket endpoints. React frontend gains new pages (Settings, enhanced Ops) and interactive preview upgrades. Communication with RK3588 hosts uses SSH via `sshpass`.

**Tech Stack:** Rust (Axum, Tokio), React 19, Tauri 2, SSH (sshpass), WebSocket (axum::extract::ws)

**Spec:** `docs/specs/2026-03-22-cluster-control-upgrade-design.md`

---

## File Structure

### Rust — New Files
| File | Responsibility |
|------|---------------|
| `src/host.rs` | SSH execution, host CRUD, container lifecycle (VmManager) |
| `src/auto_connector.rs` | Background thread: auto adb connect/reconnect |
| `src/host_monitor.rs` | Background thread: SSH stats collection, SSE push |

### Rust — Modified Files
| File | Changes |
|------|---------|
| `src/lib.rs` | Add `pub mod host; pub mod auto_connector; pub mod host_monitor;` |
| `src/bin/web.rs` | Add routes for hosts, VMs, stats, WebSocket interactive |
| `Cargo.toml` | Add `axum = { features = ["ws"] }` for WebSocket support |

### Frontend — New Files
| File | Responsibility |
|------|---------------|
| `frontend/src/pages/SettingsPage.jsx` | Host configuration UI |
| `frontend/src/hooks/useHosts.js` | Host + VM state management |
| `frontend/src/hooks/useHostStats.js` | SSE stats subscription |
| `frontend/src/hooks/useInteractive.js` | WebSocket interactive session |
| `frontend/src/components/hosts/HostDashboard.jsx` | CPU/mem/GPU/temp gauges |
| `frontend/src/components/hosts/VmTable.jsx` | Container list with actions |
| `frontend/src/components/preview/LiveControl.jsx` | Interactive touch/swipe overlay |

### Frontend — Modified Files
| File | Changes |
|------|---------|
| `frontend/src/App.jsx` | Add SettingsPage route |
| `frontend/src/components/layout/Sidebar.jsx` | Already has settings button, wire it |
| `frontend/src/pages/OpsPage.jsx` | Add "云手机" and "仪表盘" sub-tabs |
| `frontend/src/pages/ControlPage.jsx` | Add live interactive control mode |
| `frontend/src/context/AppContext.jsx` | Add hosts/VMs to global state |
| `frontend/src/api.js` | Add host/VM/stats API helpers |

---

## Phase 1: Infrastructure

### Task 1: SSH Execution Module (`host.rs`)

**Files:**
- Create: `src/host.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `host.rs` with data structures and SSH exec**

```rust
// src/host.rs
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub ssh_port: u16,
    pub user: String,
    pub password: String,
    pub adb_port_start: u16,
    pub adb_port_end: u16,
    pub image: String,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            host: String::new(),
            ssh_port: 22,
            user: "root".to_string(),
            password: "orangepi".to_string(),
            adb_port_start: 5550,
            adb_port_end: 5599,
            image: "cnflysky/redroid-rk3588:lineage-20".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct VmInfo {
    pub name: String,
    pub status: String, // "running", "exited", "created"
    pub host_port: u16,
    pub cpu_percent: String,
    pub mem_usage: String,
}

pub fn ssh_exec(config: &HostConfig, command: &str) -> Result<String, String> {
    let output = Command::new("sshpass")
        .arg("-p")
        .arg(&config.password)
        .arg("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("ConnectTimeout=5")
        .arg("-p").arg(config.ssh_port.to_string())
        .arg(format!("{}@{}", config.user, config.host))
        .arg(command)
        .output()
        .map_err(|e| format!("SSH 执行失败: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(stderr);
        }
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn test_connection(config: &HostConfig) -> Result<String, String> {
    ssh_exec(config, "echo ok && uname -a")
}

// --- Host CRUD (backed by hosts.json) ---

pub fn hosts_path(base_dir: &Path) -> std::path::PathBuf {
    base_dir.join("data").join("hosts.json")
}

pub fn load_hosts(base_dir: &Path) -> Vec<HostConfig> {
    let path = hosts_path(base_dir);
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| "[]".to_string());
    match serde_json::from_str::<Vec<HostConfig>>(&raw) {
        Ok(hosts) => hosts,
        Err(e) => {
            eprintln!("[host] 解析 hosts.json 失败: {e}");
            Vec::new()
        }
    }
}

pub fn save_hosts(base_dir: &Path, hosts: &[HostConfig]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(hosts).map_err(|e| e.to_string())?;
    crate::storage::atomic_write(&hosts_path(base_dir), format!("{json}\n").as_bytes())
}

// --- VM lifecycle ---

pub fn list_vms(config: &HostConfig) -> Result<Vec<VmInfo>, String> {
    let output = ssh_exec(
        config,
        "docker ps -a --filter 'name=redroid' --format '{{.Names}}\\t{{.Status}}\\t{{.Ports}}'"
    )?;
    let mut vms = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.is_empty() { continue; }
        let name = parts[0].trim().to_string();
        let status_raw = parts.get(1).unwrap_or(&"").trim();
        let status = if status_raw.starts_with("Up") {
            "running".to_string()
        } else if status_raw.starts_with("Exited") {
            "exited".to_string()
        } else {
            "created".to_string()
        };
        let ports_raw = parts.get(2).unwrap_or(&"").trim();
        let host_port = parse_host_port(ports_raw).unwrap_or(0);
        vms.push(VmInfo { name, status, host_port, cpu_percent: String::new(), mem_usage: String::new() });
    }
    vms.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(vms)
}

pub fn create_vms(config: &HostConfig, count: usize) -> Result<Vec<String>, String> {
    let existing = list_vms(config)?;
    let used_ports: Vec<u16> = existing.iter().map(|v| v.host_port).filter(|p| *p > 0).collect();
    let used_numbers: Vec<u32> = existing.iter()
        .filter_map(|v| v.name.strip_prefix("redroid-").and_then(|n| n.parse().ok()))
        .collect();

    let mut created = Vec::new();
    let mut next_number = 1u32;
    let mut next_port = config.adb_port_start;

    for _ in 0..count {
        while used_numbers.contains(&next_number) || (next_number == 1 && existing.iter().any(|v| v.name == "redroid")) {
            next_number += 1;
        }
        while used_ports.contains(&next_port) || next_port > config.adb_port_end {
            next_port += 1;
        }
        if next_port > config.adb_port_end {
            return Err(format!("端口范围已满 ({}-{})", config.adb_port_start, config.adb_port_end));
        }

        let name = format!("redroid-{next_number}");
        let data_dir = format!("/root/redroid-data-{next_number}");
        let cmd = format!(
            "docker run -d --name {name} --privileged \
             -p {next_port}:5555 \
             -v {data_dir}:/data \
             --memory=1536m --cpus=0.5 \
             {image} \
             androidboot.redroid_gpu_mode=host \
             androidboot.redroid_fps=30 \
             androidboot.use_memfd=1 \
             androidboot.redroid_width=720 \
             androidboot.redroid_height=1280 \
             ro.build.characteristics=default",
            image = config.image
        );
        ssh_exec(config, &cmd)?;
        created.push(name);
        next_number += 1;
        next_port += 1;
    }
    Ok(created)
}

pub fn vm_action(config: &HostConfig, name: &str, action: &str) -> Result<String, String> {
    match action {
        "start" => ssh_exec(config, &format!("docker start {name}")),
        "stop" => ssh_exec(config, &format!("docker stop {name}")),
        "restart" => ssh_exec(config, &format!("docker restart {name}")),
        _ => Err(format!("不支持的操作: {action}")),
    }
}

pub fn delete_vm(config: &HostConfig, name: &str, purge_data: bool) -> Result<String, String> {
    ssh_exec(config, &format!("docker rm -f {name}"))?;
    if purge_data {
        let num = name.strip_prefix("redroid-").unwrap_or(name);
        let data_dir = format!("/root/redroid-data-{num}");
        let _ = ssh_exec(config, &format!("rm -rf {data_dir}"));
    }
    Ok(format!("{name} 已删除"))
}

pub fn batch_vm_action(config: &HostConfig, names: &[String], action: &str) -> Vec<(String, Result<String, String>)> {
    names.iter().map(|name| {
        let result = if action == "delete" {
            delete_vm(config, name, false)
        } else {
            vm_action(config, name, action)
        };
        (name.clone(), result)
    }).collect()
}

fn parse_host_port(ports_raw: &str) -> Option<u16> {
    // Format: "0.0.0.0:5555->5555/tcp, :::5555->5555/tcp"
    ports_raw.split("->").next()
        .and_then(|s| s.rsplit(':').next())
        .and_then(|s| s.parse().ok())
}
```

- [ ] **Step 2: Export `atomic_write` from storage.rs (make it `pub`)**

In `src/storage.rs`, find `fn atomic_write` and change to `pub fn atomic_write`.

- [ ] **Step 3: Add to lib.rs**

```rust
pub mod adb;
pub mod auto_connector;
pub mod host;
pub mod host_monitor;
pub mod preview;
pub mod storage;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --bin web`
Expected: pass (auto_connector and host_monitor can be empty stubs initially)

- [ ] **Step 5: Commit**

```bash
git add src/host.rs src/lib.rs src/storage.rs
git commit -m "feat: add host.rs with SSH exec, host CRUD, VM lifecycle"
```

---

### Task 2: Auto-Connector (`auto_connector.rs`)

**Files:**
- Create: `src/auto_connector.rs`

- [ ] **Step 1: Create auto_connector.rs**

```rust
// src/auto_connector.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::host::{self, HostConfig};

const SCAN_INTERVAL: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct DeviceConnection {
    pub serial: String,
    pub host_id: String,
    pub connected: bool,
    pub last_seen: Instant,
}

pub struct AutoConnector {
    base_dir: PathBuf,
    known: RwLock<HashMap<String, DeviceConnection>>,
}

impl AutoConnector {
    pub fn new(base_dir: PathBuf) -> Arc<Self> {
        let connector = Arc::new(Self {
            base_dir,
            known: RwLock::new(HashMap::new()),
        });
        let c = connector.clone();
        thread::spawn(move || c.run_loop());
        connector
    }

    pub fn trigger_connect(&self, host: &str, port: u16) {
        let serial = format!("{host}:{port}");
        let _ = Command::new("adb")
            .arg("connect")
            .arg(&serial)
            .output();
    }

    pub fn connected_devices(&self) -> Vec<DeviceConnection> {
        self.known.read()
            .map(|k| k.values().cloned().collect())
            .unwrap_or_default()
    }

    fn run_loop(&self) {
        loop {
            if let Err(e) = self.scan_and_connect() {
                eprintln!("[auto-connector] scan error: {e}");
            }
            thread::sleep(SCAN_INTERVAL);
        }
    }

    fn scan_and_connect(&self) -> Result<(), String> {
        let hosts = host::load_hosts(&self.base_dir);
        if hosts.is_empty() {
            return Ok(());
        }

        // Collect expected serials from all hosts
        let mut expected: HashMap<String, String> = HashMap::new(); // serial -> host_id
        for h in &hosts {
            match host::list_vms(h) {
                Ok(vms) => {
                    for vm in vms.iter().filter(|v| v.status == "running" && v.host_port > 0) {
                        let serial = format!("{}:{}", h.host, vm.host_port);
                        expected.insert(serial, h.id.clone());
                    }
                }
                Err(e) => eprintln!("[auto-connector] 查询 {} 失败: {e}", h.name),
            }
        }

        // Get current adb devices
        let current = self.adb_devices();

        // Connect missing, reconnect offline
        for (serial, host_id) in &expected {
            let status = current.get(serial.as_str());
            match status {
                None => {
                    // Not connected at all
                    let _ = Command::new("adb").arg("connect").arg(serial).output();
                }
                Some(state) if state != "device" => {
                    // offline/unauthorized — disconnect then reconnect
                    let _ = Command::new("adb").arg("disconnect").arg(serial).output();
                    thread::sleep(Duration::from_millis(200));
                    let _ = Command::new("adb").arg("connect").arg(serial).output();
                }
                _ => {} // already connected
            }

            if let Ok(mut known) = self.known.write() {
                known.insert(serial.clone(), DeviceConnection {
                    serial: serial.clone(),
                    host_id: host_id.clone(),
                    connected: true,
                    last_seen: Instant::now(),
                });
            }
        }

        Ok(())
    }

    fn adb_devices(&self) -> HashMap<String, String> {
        let output = Command::new("adb")
            .arg("devices")
            .arg("-l")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let mut map = HashMap::new();
        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                map.insert(parts[0].to_string(), parts[1].to_string());
            }
        }
        map
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --bin web`

- [ ] **Step 3: Commit**

```bash
git add src/auto_connector.rs
git commit -m "feat: add auto_connector with background scan and reconnect"
```

---

### Task 3: Host Monitor (`host_monitor.rs`)

**Files:**
- Create: `src/host_monitor.rs`

- [ ] **Step 1: Create host_monitor.rs**

```rust
// src/host_monitor.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use std::path::PathBuf;

use serde::Serialize;
use crate::host::{self, HostConfig};

const COLLECT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default, Serialize)]
pub struct HostStats {
    pub host_id: String,
    pub cpu_usage: String,
    pub load_average: String,
    pub mem_total_mb: u64,
    pub mem_used_mb: u64,
    pub swap_used_mb: u64,
    pub temperature: f32,
    pub gpu_freq_mhz: u32,
    pub gpu_load: String,
    pub containers: Vec<ContainerStats>,
    pub collected_at_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ContainerStats {
    pub name: String,
    pub cpu_percent: String,
    pub mem_usage: String,
}

pub struct HostMonitor {
    base_dir: PathBuf,
    snapshots: RwLock<HashMap<String, HostStats>>,
}

impl HostMonitor {
    pub fn new(base_dir: PathBuf) -> Arc<Self> {
        let monitor = Arc::new(Self {
            base_dir,
            snapshots: RwLock::new(HashMap::new()),
        });
        let m = monitor.clone();
        thread::spawn(move || m.run_loop());
        monitor
    }

    pub fn get_stats(&self, host_id: &str) -> Option<HostStats> {
        self.snapshots.read().ok()?.get(host_id).cloned()
    }

    pub fn all_stats(&self) -> Vec<HostStats> {
        self.snapshots.read()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default()
    }

    fn run_loop(&self) {
        loop {
            let hosts = host::load_hosts(&self.base_dir);
            for h in &hosts {
                match self.collect(h) {
                    Ok(stats) => {
                        if let Ok(mut snapshots) = self.snapshots.write() {
                            snapshots.insert(h.id.clone(), stats);
                        }
                    }
                    Err(e) => eprintln!("[monitor] {} 采集失败: {e}", h.name),
                }
            }
            thread::sleep(COLLECT_INTERVAL);
        }
    }

    fn collect(&self, config: &HostConfig) -> Result<HostStats, String> {
        let cmd = concat!(
            "echo \"LOAD:$(cat /proc/loadavg)\";",
            "echo \"MEM:$(free -m | grep Mem)\";",
            "echo \"SWAP:$(free -m | grep Swap)\";",
            "echo \"TEMP:$(cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null)\";",
            "echo \"GPU_FREQ:$(cat /sys/class/devfreq/fb000000.gpu/cur_freq 2>/dev/null)\";",
            "echo \"GPU_LOAD:$(cat /sys/class/devfreq/fb000000.gpu/load 2>/dev/null)\";",
            "docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}' 2>/dev/null"
        );
        let output = host::ssh_exec(config, cmd)?;
        Ok(parse_stats(&config.id, &output))
    }
}

fn parse_stats(host_id: &str, raw: &str) -> HostStats {
    let mut stats = HostStats { host_id: host_id.to_string(), ..Default::default() };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    stats.collected_at_ms = now;

    for line in raw.lines() {
        if let Some(val) = line.strip_prefix("LOAD:") {
            stats.load_average = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("MEM:") {
            let parts: Vec<&str> = val.split_whitespace().collect();
            stats.mem_total_mb = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            stats.mem_used_mb = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("SWAP:") {
            let parts: Vec<&str> = val.split_whitespace().collect();
            stats.swap_used_mb = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("TEMP:") {
            stats.temperature = val.trim().parse::<f32>().unwrap_or(0.0) / 1000.0;
        } else if let Some(val) = line.strip_prefix("GPU_FREQ:") {
            stats.gpu_freq_mhz = val.trim().parse::<u64>().unwrap_or(0) as u32 / 1_000_000;
        } else if let Some(val) = line.strip_prefix("GPU_LOAD:") {
            stats.gpu_load = val.trim().to_string();
        } else if line.contains('|') && !line.starts_with("LOAD:") {
            // docker stats line: name|cpu%|mem
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                stats.containers.push(ContainerStats {
                    name: parts[0].trim().to_string(),
                    cpu_percent: parts[1].trim().to_string(),
                    mem_usage: parts[2].trim().to_string(),
                });
            }
        }
    }
    stats.containers.sort_by(|a, b| a.name.cmp(&b.name));
    stats
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --bin web`

- [ ] **Step 3: Commit**

```bash
git add src/host_monitor.rs
git commit -m "feat: add host_monitor with SSH stats collection"
```

---

### Task 4: Web API Routes — Hosts, VMs, Stats

**Files:**
- Modify: `src/bin/web.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add WebSocket feature to Cargo.toml**

Change the axum dependency:
```toml
axum = { version = "0.8.6", features = ["ws"] }
```

- [ ] **Step 2: Add imports and state fields to web.rs**

At the top of `web.rs`, add imports:
```rust
use android_group_control_rs::host::{self, HostConfig};
use android_group_control_rs::auto_connector::AutoConnector;
use android_group_control_rs::host_monitor::HostMonitor;
```

Add to `AppState`:
```rust
struct AppState {
    base_dir: PathBuf,
    focus_mirror: Mutex<FocusMirrorState>,
    preview_hub: PreviewHub,
    auto_connector: Arc<AutoConnector>,
    host_monitor: Arc<HostMonitor>,
}
```

In `main()`, initialize them:
```rust
let auto_connector = AutoConnector::new(base_dir.clone());
let host_monitor = HostMonitor::new(base_dir.clone());

let state = Arc::new(AppState {
    base_dir,
    focus_mirror: Mutex::new(FocusMirrorState::default()),
    preview_hub: PreviewHub::new(),
    auto_connector,
    host_monitor,
});
```

- [ ] **Step 3: Add host CRUD routes**

Add to the router:
```rust
.route("/api/hosts", get(get_hosts).post(save_host))
.route("/api/hosts/{id}", axum::routing::put(update_host).delete(delete_host))
.route("/api/hosts/{id}/test", post(test_host_connection))
.route("/api/hosts/{id}/vms", get(get_vms).post(create_vms))
.route("/api/hosts/{id}/vms/{name}/start", post(start_vm))
.route("/api/hosts/{id}/vms/{name}/stop", post(stop_vm))
.route("/api/hosts/{id}/vms/{name}/restart", post(restart_vm))
.route("/api/hosts/{id}/vms/{name}", delete(delete_vm_handler))
.route("/api/hosts/{id}/vms/batch", post(batch_vm_action_handler))
.route("/api/hosts/{id}/stats", get(get_host_stats))
.route("/api/hosts/{id}/stats/stream", get(stream_host_stats))
```

- [ ] **Step 4: Implement host handlers**

Add these handler functions to `web.rs`:

```rust
// --- Request/Response types ---
#[derive(Deserialize)]
struct SaveHostRequest {
    name: String,
    host: String,
    ssh_port: Option<u16>,
    user: Option<String>,
    password: String,
    adb_port_start: Option<u16>,
    adb_port_end: Option<u16>,
    image: Option<String>,
}

#[derive(Deserialize)]
struct CreateVmsRequest {
    count: usize,
}

#[derive(Deserialize)]
struct BatchVmRequest {
    names: Vec<String>,
    action: String, // "start", "stop", "restart", "delete"
}

// --- Handlers ---
async fn get_hosts(State(state): State<Arc<AppState>>) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        Ok(Json(serde_json::json!({ "hosts": hosts })))
    }).await
}

async fn save_host(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveHostRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut hosts = host::load_hosts(&base_dir);
        let id = storage::make_id("host");
        hosts.push(HostConfig {
            id: id.clone(),
            name: payload.name.trim().to_string(),
            host: payload.host.trim().to_string(),
            ssh_port: payload.ssh_port.unwrap_or(22),
            user: payload.user.unwrap_or_else(|| "root".to_string()),
            password: payload.password,
            adb_port_start: payload.adb_port_start.unwrap_or(5550),
            adb_port_end: payload.adb_port_end.unwrap_or(5599),
            image: payload.image.unwrap_or_else(|| "cnflysky/redroid-rk3588:lineage-20".to_string()),
        });
        host::save_hosts(&base_dir, &hosts).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "hosts": hosts })))
    }).await
}

async fn update_host(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<SaveHostRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut hosts = host::load_hosts(&base_dir);
        let h = hosts.iter_mut().find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        h.name = payload.name.trim().to_string();
        h.host = payload.host.trim().to_string();
        h.ssh_port = payload.ssh_port.unwrap_or(22);
        h.password = payload.password;
        if let Some(u) = payload.user { h.user = u; }
        if let Some(s) = payload.adb_port_start { h.adb_port_start = s; }
        if let Some(e) = payload.adb_port_end { h.adb_port_end = e; }
        if let Some(i) = payload.image { h.image = i; }
        host::save_hosts(&base_dir, &hosts).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "hosts": hosts })))
    }).await
}

async fn delete_host(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let mut hosts = host::load_hosts(&base_dir);
        hosts.retain(|h| h.id != id);
        host::save_hosts(&base_dir, &hosts).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "hosts": hosts })))
    }).await
}

async fn test_host_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let h = hosts.iter().find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        let result = host::test_connection(h).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "ok": true, "output": result })))
    }).await
}

async fn get_vms(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let h = hosts.iter().find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        let vms = host::list_vms(h).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "vms": vms })))
    }).await
}

async fn create_vms(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<CreateVmsRequest>,
) -> Result<Json<Value>, ApiError> {
    if payload.count == 0 || payload.count > 20 {
        return Err(ApiError::bad_request("创建数量需要在 1-20 之间"));
    }
    let base_dir = state.base_dir.clone();
    let auto_connector = state.auto_connector.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let h = hosts.iter().find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        let created = host::create_vms(h, payload.count).map_err(ApiError::bad_request)?;
        // Wait for containers to boot, then auto-connect
        std::thread::sleep(std::time::Duration::from_secs(10));
        let vms = host::list_vms(h).map_err(ApiError::bad_request)?;
        for vm in vms.iter().filter(|v| v.status == "running" && v.host_port > 0) {
            auto_connector.trigger_connect(&h.host, vm.host_port);
        }
        Ok(Json(serde_json::json!({ "created": created, "vms": vms })))
    }).await
}

async fn start_vm(State(state): State<Arc<AppState>>, Path((id, name)): Path<(String, String)>) -> Result<Json<Value>, ApiError> {
    vm_action_handler(state, &id, &name, "start").await
}
async fn stop_vm(State(state): State<Arc<AppState>>, Path((id, name)): Path<(String, String)>) -> Result<Json<Value>, ApiError> {
    vm_action_handler(state, &id, &name, "stop").await
}
async fn restart_vm(State(state): State<Arc<AppState>>, Path((id, name)): Path<(String, String)>) -> Result<Json<Value>, ApiError> {
    vm_action_handler(state, &id, &name, "restart").await
}

async fn vm_action_handler(state: Arc<AppState>, host_id: &str, vm_name: &str, action: &str) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    let host_id = host_id.to_string();
    let vm_name = vm_name.to_string();
    let action = action.to_string();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let h = hosts.iter().find(|h| h.id == host_id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        host::vm_action(h, &vm_name, &action).map_err(ApiError::bad_request)?;
        let vms = host::list_vms(h).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "vms": vms })))
    }).await
}

async fn delete_vm_handler(State(state): State<Arc<AppState>>, Path((id, name)): Path<(String, String)>) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let h = hosts.iter().find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        host::delete_vm(h, &name, false).map_err(ApiError::bad_request)?;
        let vms = host::list_vms(h).map_err(ApiError::bad_request)?;
        Ok(Json(serde_json::json!({ "vms": vms })))
    }).await
}

async fn batch_vm_action_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<BatchVmRequest>,
) -> Result<Json<Value>, ApiError> {
    let base_dir = state.base_dir.clone();
    run_blocking(move || {
        let hosts = host::load_hosts(&base_dir);
        let h = hosts.iter().find(|h| h.id == id)
            .ok_or_else(|| ApiError::bad_request("宿主机不存在"))?;
        let results = host::batch_vm_action(h, &payload.names, &payload.action);
        let vms = host::list_vms(h).map_err(ApiError::bad_request)?;
        let errors: Vec<_> = results.iter().filter_map(|(n, r)| r.as_ref().err().map(|e| format!("{n}: {e}"))).collect();
        Ok(Json(serde_json::json!({ "vms": vms, "errors": errors })))
    }).await
}

async fn get_host_stats(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let stats = state.host_monitor.get_stats(&id);
    Ok(Json(serde_json::json!({ "stats": stats })))
}

async fn stream_host_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let monitor = state.host_monitor.clone();
    let body_stream = stream! {
        loop {
            if let Some(stats) = monitor.get_stats(&id) {
                if let Ok(json) = serde_json::to_string(&stats) {
                    let chunk = format!("event: stats\ndata: {json}\n\n");
                    yield Ok::<Bytes, Infallible>(Bytes::from(chunk.into_bytes()));
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };
    let mut response = Response::new(Body::from_stream(body_stream));
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --bin web`

- [ ] **Step 6: Commit**

```bash
git add src/bin/web.rs Cargo.toml
git commit -m "feat: add host/VM/stats API routes to web server"
```

---

### Task 5: Frontend — API Helpers and Hosts Hook

**Files:**
- Modify: `frontend/src/api.js`
- Create: `frontend/src/hooks/useHosts.js`
- Create: `frontend/src/hooks/useHostStats.js`

- [ ] **Step 1: Add host/VM API methods to api.js**

Append to the `api` object in `api.js`:
```javascript
// Add to api.js exports
export const hostApi = {
  getHosts: () => api.get("/api/hosts"),
  addHost: (data) => api.post("/api/hosts", data),
  updateHost: (id, data) => api.post(`/api/hosts/${id}`, { ...data, _method: "PUT" }), // or use PUT
  deleteHost: (id) => api.delete(`/api/hosts/${id}`),
  testHost: (id) => api.post(`/api/hosts/${id}/test`),
  getVms: (hostId) => api.get(`/api/hosts/${hostId}/vms`),
  createVms: (hostId, count) => api.post(`/api/hosts/${hostId}/vms`, { count }),
  startVm: (hostId, name) => api.post(`/api/hosts/${hostId}/vms/${name}/start`),
  stopVm: (hostId, name) => api.post(`/api/hosts/${hostId}/vms/${name}/stop`),
  restartVm: (hostId, name) => api.post(`/api/hosts/${hostId}/vms/${name}/restart`),
  deleteVm: (hostId, name) => api.delete(`/api/hosts/${hostId}/vms/${name}`),
  batchVmAction: (hostId, names, action) => api.post(`/api/hosts/${hostId}/vms/batch`, { names, action }),
  getStats: (hostId) => api.get(`/api/hosts/${hostId}/stats`),
};
```

- [ ] **Step 2: Create useHosts.js hook**

```javascript
// frontend/src/hooks/useHosts.js
import { useState, useEffect, useCallback } from "react";
import { hostApi } from "../api";

export function useHosts() {
  const [hosts, setHosts] = useState([]);
  const [activeHostId, setActiveHostId] = useState(null);
  const [vms, setVms] = useState([]);
  const [loading, setLoading] = useState(false);

  const refreshHosts = useCallback(async () => {
    try {
      const data = await hostApi.getHosts();
      setHosts(data.hosts || []);
      if (!activeHostId && data.hosts?.length) {
        setActiveHostId(data.hosts[0].id);
      }
    } catch (e) { console.error("refreshHosts", e); }
  }, [activeHostId]);

  const refreshVms = useCallback(async () => {
    if (!activeHostId) return;
    setLoading(true);
    try {
      const data = await hostApi.getVms(activeHostId);
      setVms(data.vms || []);
    } catch (e) { console.error("refreshVms", e); }
    setLoading(false);
  }, [activeHostId]);

  useEffect(() => { refreshHosts(); }, []);
  useEffect(() => { if (activeHostId) refreshVms(); }, [activeHostId]);

  const addHost = async (config) => {
    const data = await hostApi.addHost(config);
    setHosts(data.hosts || []);
  };
  const deleteHost = async (id) => {
    const data = await hostApi.deleteHost(id);
    setHosts(data.hosts || []);
  };
  const testHost = (id) => hostApi.testHost(id);
  const createVms = async (count) => {
    if (!activeHostId) return;
    const data = await hostApi.createVms(activeHostId, count);
    setVms(data.vms || []);
  };
  const vmAction = async (name, action) => {
    if (!activeHostId) return;
    const fn = action === "start" ? hostApi.startVm
      : action === "stop" ? hostApi.stopVm
      : action === "restart" ? hostApi.restartVm
      : action === "delete" ? hostApi.deleteVm : null;
    if (!fn) return;
    const data = await fn(activeHostId, name);
    if (data.vms) setVms(data.vms);
  };
  const batchAction = async (names, action) => {
    if (!activeHostId) return;
    const data = await hostApi.batchVmAction(activeHostId, names, action);
    if (data.vms) setVms(data.vms);
  };

  return {
    hosts, activeHostId, setActiveHostId, vms, loading,
    refreshHosts, refreshVms, addHost, deleteHost, testHost,
    createVms, vmAction, batchAction,
  };
}
```

- [ ] **Step 3: Create useHostStats.js hook**

```javascript
// frontend/src/hooks/useHostStats.js
import { useState, useEffect, useRef } from "react";
import { apiUrl } from "../api";

export function useHostStats(hostId) {
  const [stats, setStats] = useState(null);
  const sourceRef = useRef(null);

  useEffect(() => {
    if (!hostId) return;
    const es = new EventSource(apiUrl(`/api/hosts/${hostId}/stats/stream`));
    sourceRef.current = es;
    es.addEventListener("stats", (e) => {
      try { setStats(JSON.parse(e.data)); } catch {}
    });
    es.onerror = () => { setStats(null); };
    return () => { es.close(); sourceRef.current = null; };
  }, [hostId]);

  return stats;
}
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/api.js frontend/src/hooks/useHosts.js frontend/src/hooks/useHostStats.js
git commit -m "feat: add host/VM API helpers and React hooks"
```

---

### Task 6: Frontend — Settings Page + OpsPage Upgrade

**Files:**
- Create: `frontend/src/pages/SettingsPage.jsx`
- Create: `frontend/src/components/hosts/HostDashboard.jsx`
- Create: `frontend/src/components/hosts/VmTable.jsx`
- Modify: `frontend/src/pages/OpsPage.jsx`
- Modify: `frontend/src/App.jsx`

- [ ] **Step 1: Create SettingsPage.jsx**

Host configuration page where users add/edit/remove RK3588 hosts.
(File: `frontend/src/pages/SettingsPage.jsx` — settings form for hosts CRUD with test connection button)

- [ ] **Step 2: Create HostDashboard.jsx**

Dashboard component showing CPU, memory, GPU, temperature gauges.
(File: `frontend/src/components/hosts/HostDashboard.jsx` — stat cards using useHostStats)

- [ ] **Step 3: Create VmTable.jsx**

Cloud phone list with start/stop/restart/delete buttons and batch actions.
(File: `frontend/src/components/hosts/VmTable.jsx` — table with action buttons, create N input)

- [ ] **Step 4: Update OpsPage.jsx**

Add "云手机" and "仪表盘" sub-tabs alongside existing tabs. Import HostDashboard and VmTable.

- [ ] **Step 5: Update App.jsx**

Add `SettingsPage` to the page router:
```jsx
{activePage === "settings" && <SettingsPage />}
```

- [ ] **Step 6: Verify frontend builds**

Run: `cd frontend && npm run build`

- [ ] **Step 7: Commit**

```bash
git add frontend/src/
git commit -m "feat: add SettingsPage, HostDashboard, VmTable, OpsPage upgrade"
```

---

## Phase 2: Interactive Control

### Task 7: WebSocket Interactive Endpoint

**Files:**
- Modify: `src/bin/web.rs`

- [ ] **Step 1: Add WebSocket handler**

Add route:
```rust
.route("/api/interactive/{serial}", get(ws_interactive))
.route("/api/interactive/sync", get(ws_interactive_sync))
```

Implement WebSocket handler that receives tap/swipe/keyevent/text JSON messages and dispatches them via `adb::fast_shell_command`, while pushing preview frames back.

- [ ] **Step 2: Verify compilation**

Run: `cargo check --bin web`

- [ ] **Step 3: Commit**

```bash
git add src/bin/web.rs
git commit -m "feat: add WebSocket interactive control endpoint"
```

---

### Task 8: Frontend — Live Interactive Control

**Files:**
- Create: `frontend/src/components/preview/LiveControl.jsx`
- Create: `frontend/src/hooks/useInteractive.js`
- Modify: `frontend/src/pages/ControlPage.jsx`

- [ ] **Step 1: Create useInteractive.js**

WebSocket hook that manages connection to `/api/interactive/{serial}`, sends touch events, receives frames.

- [ ] **Step 2: Create LiveControl.jsx**

Overlay component on device preview that captures mouse/touch events, maps coordinates to device resolution (720x1280), sends via WebSocket. Supports:
- Click: mousedown+mouseup < 300ms, distance < 10px → tap
- Swipe: mousedown → mousemove → mouseup → swipe with duration
- Long press: mousedown > 500ms without move → long press

Also receives JPEG frames from WebSocket and displays them as the live preview.

- [ ] **Step 3: Update ControlPage.jsx**

Add "实时操控" mode that shows LiveControl for the focused device. When multiple devices are selected, show sync mode toggle — operations on the focused device broadcast to all selected.

- [ ] **Step 4: Verify frontend builds**

Run: `cd frontend && npm run build`

- [ ] **Step 5: Commit**

```bash
git add frontend/src/
git commit -m "feat: add live interactive control with WebSocket"
```

---

### Task 9: Recording from Interactive Mode

**Files:**
- Modify: `frontend/src/pages/ControlPage.jsx`
- Modify: `frontend/src/hooks/useInteractive.js`

- [ ] **Step 1: Add recording state to useInteractive**

When recording is active, every outgoing WebSocket message is also pushed to a `recordedSteps` array with timestamp deltas.

- [ ] **Step 2: Add record/stop/save UI to ControlPage**

- "开始录制" button starts capturing
- "停止录制" button stops, shows step count
- "保存录制" button POSTs to `/api/recordings` with the captured steps
- "回放" button POSTs to `/api/recordings/{id}/run` with selected serials

- [ ] **Step 3: Verify frontend builds**

Run: `cd frontend && npm run build`

- [ ] **Step 4: Commit**

```bash
git add frontend/src/
git commit -m "feat: add interactive recording mode"
```

---

### Task 10: Integration Test + Final Verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test`
Expected: all existing tests pass, no regressions

- [ ] **Step 2: Run frontend tests**

Run: `cd frontend && npm test`

- [ ] **Step 3: Run full build**

Run: `cargo build --bin web && cd frontend && npm run build`

- [ ] **Step 4: Manual smoke test**

1. Start web: `cargo run --bin web`
2. Start frontend: `cd frontend && npm run dev`
3. Open browser → Settings → Add host (192.168.31.9)
4. Go to OpsPage → 云手机 tab → verify containers listed
5. Go to OpsPage → 仪表盘 tab → verify stats streaming
6. Go to ControlPage → open interactive mode → tap/swipe device
7. Test recording: record → stop → save → replay

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete cluster control upgrade — hosts, VMs, monitoring, interactive control"
```
