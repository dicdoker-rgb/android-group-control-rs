use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::host;

const SCAN_INTERVAL: Duration = Duration::from_secs(30);

/// Tracks the connection state for a single device (Redroid container).
#[derive(Clone, Debug)]
pub struct DeviceConnection {
    pub serial: String,
    pub host_id: String,
    pub connected: bool,
    pub last_seen: Instant,
}

/// Background service that periodically discovers running Redroid containers
/// across all configured hosts and ensures they are `adb connect`-ed.
pub struct AutoConnector {
    base_dir: PathBuf,
    known: RwLock<HashMap<String, DeviceConnection>>,
}

impl AutoConnector {
    /// Create a new `AutoConnector` and spawn its background scan thread.
    ///
    /// The returned `Arc` can be shared freely; the background thread holds
    /// its own clone and will run until the process exits.
    pub fn new(base_dir: PathBuf) -> Arc<Self> {
        let ac = Arc::new(Self {
            base_dir,
            known: RwLock::new(HashMap::new()),
        });

        let handle = Arc::clone(&ac);
        thread::Builder::new()
            .name("auto-connector".into())
            .spawn(move || handle.run_loop())
            .expect("failed to spawn auto-connector thread");

        ac
    }

    /// Immediately attempt to `adb connect` a single device.
    pub fn trigger_connect(&self, host: &str, port: u16) {
        let serial = format!("{}:{}", host, port);
        let result = Self::run_adb_connect(&serial);
        match result {
            Ok(output) => {
                eprintln!("[auto_connector] trigger_connect {}: {}", serial, output);
                let mut map = self.known.write().unwrap_or_else(|e| e.into_inner());
                map.insert(
                    serial.clone(),
                    DeviceConnection {
                        serial,
                        host_id: host.to_string(),
                        connected: true,
                        last_seen: Instant::now(),
                    },
                );
            }
            Err(e) => {
                eprintln!("[auto_connector] trigger_connect {} failed: {}", serial, e);
            }
        }
    }

    /// Return a snapshot of all currently known device connections.
    pub fn connected_devices(&self) -> Vec<DeviceConnection> {
        let map = self.known.read().unwrap_or_else(|e| e.into_inner());
        map.values().cloned().collect()
    }

    // ── private ──────────────────────────────────────────────────────

    /// Main background loop — runs scan_and_connect every SCAN_INTERVAL.
    fn run_loop(&self) {
        loop {
            if let Err(e) = self.scan_and_connect() {
                eprintln!("[auto_connector] scan cycle error: {}", e);
            }
            thread::sleep(SCAN_INTERVAL);
        }
    }

    /// Execute one full scan cycle:
    ///
    /// 1. Load hosts from disk.
    /// 2. For each host, list running VMs via SSH.
    /// 3. Parse current `adb devices` output.
    /// 4. Connect any running container not yet in adb.
    /// 5. Re-connect any device that shows offline/unauthorized.
    /// 6. Update the known-devices map.
    fn scan_and_connect(&self) -> Result<(), String> {
        let hosts = host::load_hosts(&self.base_dir);
        if hosts.is_empty() {
            // No hosts configured — nothing to do.
            return Ok(());
        }

        let current_adb = self.adb_devices();
        let mut updated: HashMap<String, DeviceConnection> = HashMap::new();

        for h in &hosts {
            let vms = match host::list_vms(h) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "[auto_connector] failed to list VMs on host {}: {}",
                        h.id, e
                    );
                    continue;
                }
            };

            for vm in &vms {
                let serial = format!("{}:{}", h.host, vm.host_port);

                match current_adb.get(&serial).map(|s| s.as_str()) {
                    Some("device") => {
                        // Already connected and healthy — nothing to do.
                        updated.insert(
                            serial.clone(),
                            DeviceConnection {
                                serial,
                                host_id: h.id.clone(),
                                connected: true,
                                last_seen: Instant::now(),
                            },
                        );
                    }
                    Some("offline") | Some("unauthorized") => {
                        // Bad state — disconnect then reconnect.
                        eprintln!(
                            "[auto_connector] device {} in bad state, reconnecting",
                            serial
                        );
                        let _ = Self::run_adb_disconnect(&serial);
                        thread::sleep(Duration::from_millis(500));
                        let ok = Self::run_adb_connect(&serial).is_ok();
                        updated.insert(
                            serial.clone(),
                            DeviceConnection {
                                serial,
                                host_id: h.id.clone(),
                                connected: ok,
                                last_seen: Instant::now(),
                            },
                        );
                    }
                    _ => {
                        // Not present in adb at all — connect it.
                        let ok = match Self::run_adb_connect(&serial) {
                            Ok(output) => {
                                eprintln!("[auto_connector] adb connect {} => {}", serial, output);
                                true
                            }
                            Err(e) => {
                                eprintln!("[auto_connector] adb connect {} failed: {}", serial, e);
                                false
                            }
                        };
                        updated.insert(
                            serial.clone(),
                            DeviceConnection {
                                serial,
                                host_id: h.id.clone(),
                                connected: ok,
                                last_seen: Instant::now(),
                            },
                        );
                    }
                }
            }
        }

        // Swap in the new map.
        let mut map = self.known.write().unwrap_or_else(|e| e.into_inner());
        *map = updated;
        Ok(())
    }

    /// Parse the output of `adb devices -l` into a map of serial -> state.
    ///
    /// Example line: `192.168.1.100:5555  device product:...`
    /// We extract: key = "192.168.1.100:5555", value = "device"
    fn adb_devices(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();

        let output = match Command::new("adb").args(["devices", "-l"]).output() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[auto_connector] failed to run `adb devices -l`: {}", e);
                return result;
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            // Skip the header and blank lines.
            if line.starts_with("List of") || line.trim().is_empty() {
                continue;
            }
            // Format: "<serial>\t<state> ..."  or  "<serial>  <state> ..."
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                result.insert(parts[0].to_string(), parts[1].to_string());
            }
        }

        result
    }

    // ── adb helpers ──────────────────────────────────────────────────

    fn run_adb_connect(serial: &str) -> Result<String, String> {
        let output = Command::new("adb")
            .args(["connect", serial])
            .output()
            .map_err(|e| format!("adb connect exec error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() && !stdout.contains("connected") {
            return Err(format!("adb connect failed: {} {}", stdout, stderr));
        }
        Ok(stdout)
    }

    fn run_adb_disconnect(serial: &str) -> Result<String, String> {
        let output = Command::new("adb")
            .args(["disconnect", serial])
            .output()
            .map_err(|e| format!("adb disconnect exec error: {}", e))?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
