use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::host::{self, HostConfig};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// HostMonitor
// ---------------------------------------------------------------------------

pub struct HostMonitor {
    base_dir: PathBuf,
    snapshots: RwLock<HashMap<String, HostStats>>,
}

impl HostMonitor {
    /// Create a new monitor and spawn the background collection thread.
    pub fn new(base_dir: PathBuf) -> Arc<Self> {
        let monitor = Arc::new(Self {
            base_dir,
            snapshots: RwLock::new(HashMap::new()),
        });
        let m = Arc::clone(&monitor);
        thread::spawn(move || m.run_loop());
        monitor
    }

    /// Return the latest snapshot for a single host.
    pub fn get_stats(&self, host_id: &str) -> Option<HostStats> {
        self.snapshots
            .read()
            .ok()
            .and_then(|map| map.get(host_id).cloned())
    }

    /// Return all current snapshots.
    pub fn all_stats(&self) -> Vec<HostStats> {
        self.snapshots
            .read()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // Background loop
    // -----------------------------------------------------------------------

    fn run_loop(&self) {
        loop {
            let hosts = host::load_hosts(&self.base_dir);
            if hosts.is_empty() {
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            for cfg in &hosts {
                match self.collect(cfg) {
                    Ok(stats) => {
                        if let Ok(mut map) = self.snapshots.write() {
                            map.insert(stats.host_id.clone(), stats);
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[host_monitor] failed to collect stats from {}: {}",
                            cfg.id, e
                        );
                    }
                }
            }

            thread::sleep(Duration::from_secs(5));
        }
    }

    // -----------------------------------------------------------------------
    // Collection via SSH
    // -----------------------------------------------------------------------

    fn collect(&self, config: &HostConfig) -> Result<HostStats, String> {
        let remote_cmd = concat!(
            r#"echo "LOAD:$(cat /proc/loadavg)";"#,
            r#"echo "MEM:$(free -m | grep Mem)";"#,
            r#"echo "SWAP:$(free -m | grep Swap)";"#,
            r#"echo "TEMP:$(cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null)";"#,
            r#"echo "GPU_FREQ:$(cat /sys/class/devfreq/fb000000.gpu/cur_freq 2>/dev/null)";"#,
            r#"echo "GPU_LOAD:$(cat /sys/class/devfreq/fb000000.gpu/load 2>/dev/null)";"#,
            r#"docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}' 2>/dev/null"#,
        );

        let raw = host::ssh_exec(config, remote_cmd)?;
        Ok(Self::parse_stats(&config.id, &raw))
    }

    // -----------------------------------------------------------------------
    // Parsing
    // -----------------------------------------------------------------------

    fn parse_stats(host_id: &str, raw: &str) -> HostStats {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut stats = HostStats {
            host_id: host_id.to_string(),
            collected_at_ms: now_ms,
            ..Default::default()
        };

        let mut containers: Vec<ContainerStats> = Vec::new();

        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(rest) = line.strip_prefix("LOAD:") {
                stats.load_average = rest.trim().to_string();
                // Derive a simple CPU-usage hint from the 1-min load average.
                if let Some(first) = rest.trim().split_whitespace().next() {
                    stats.cpu_usage = first.to_string();
                }
            } else if let Some(rest) = line.strip_prefix("MEM:") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                // Expected: Mem: <total> <used> ...
                if parts.len() >= 3 {
                    stats.mem_total_mb = parts[1].parse().unwrap_or(0);
                    stats.mem_used_mb = parts[2].parse().unwrap_or(0);
                }
            } else if let Some(rest) = line.strip_prefix("SWAP:") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 3 {
                    stats.swap_used_mb = parts[2].parse().unwrap_or(0);
                }
            } else if let Some(rest) = line.strip_prefix("TEMP:") {
                let v: f32 = rest.trim().parse().unwrap_or(0.0);
                stats.temperature = v / 1000.0;
            } else if let Some(rest) = line.strip_prefix("GPU_FREQ:") {
                let v: u64 = rest.trim().parse().unwrap_or(0);
                stats.gpu_freq_mhz = (v / 1_000_000) as u32;
            } else if let Some(rest) = line.strip_prefix("GPU_LOAD:") {
                stats.gpu_load = rest.trim().to_string();
            } else if line.contains('|') {
                // Docker stats line: name|cpu%|mem_usage
                let fields: Vec<&str> = line.splitn(3, '|').collect();
                if fields.len() == 3 {
                    containers.push(ContainerStats {
                        name: fields[0].trim().to_string(),
                        cpu_percent: fields[1].trim().to_string(),
                        mem_usage: fields[2].trim().to_string(),
                    });
                }
            }
        }

        containers.sort_by(|a, b| a.name.cmp(&b.name));
        stats.containers = containers;

        stats
    }
}
