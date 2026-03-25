use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

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
    pub status: String,
    pub host_port: u16,
    pub cpu_percent: String,
    pub mem_usage: String,
}

// ---------------------------------------------------------------------------
// SSH execution
// ---------------------------------------------------------------------------

pub fn ssh_exec(config: &HostConfig, command: &str) -> Result<String, String> {
    let output = Command::new("sshpass")
        .arg("-p")
        .arg(&config.password)
        .arg("ssh")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg("-p")
        .arg(config.ssh_port.to_string())
        .arg(format!("{}@{}", config.user, config.host))
        .arg(command)
        .output()
        .map_err(|e| format!("SSH 执行失败: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "SSH 命令失败 (exit {}): {}{}",
            output.status.code().unwrap_or(-1),
            stderr,
            stdout
        ))
    }
}

pub fn test_connection(config: &HostConfig) -> Result<String, String> {
    ssh_exec(config, "echo ok && uname -a")
}

// ---------------------------------------------------------------------------
// Host CRUD (JSON persistence)
// ---------------------------------------------------------------------------

pub fn hosts_path(base_dir: &Path) -> PathBuf {
    base_dir.join("data").join("hosts.json")
}

pub fn load_hosts(base_dir: &Path) -> Vec<HostConfig> {
    let path = hosts_path(base_dir);
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| "[]".to_string());
    match serde_json::from_str::<Vec<HostConfig>>(&raw) {
        Ok(items) => items,
        Err(err) => {
            eprintln!(
                "[host] 解析 {} 失败: {err}，返回空列表",
                path.display()
            );
            Vec::new()
        }
    }
}

pub fn save_hosts(base_dir: &Path, hosts: &[HostConfig]) -> Result<(), String> {
    let path = hosts_path(base_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(hosts).map_err(|e| e.to_string())?;
    crate::storage::atomic_write(&path, format!("{json}\n").as_bytes())
}

// ---------------------------------------------------------------------------
// VM lifecycle
// ---------------------------------------------------------------------------

pub fn list_vms(config: &HostConfig) -> Result<Vec<VmInfo>, String> {
    let output = ssh_exec(
        config,
        "docker ps -a --filter 'name=redroid' --format '{{.Names}}\t{{.Status}}\t{{.Ports}}'",
    )?;

    let mut vms = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let name = parts[0].to_string();
        let raw_status = parts[1].to_string();
        let ports_str = if parts.len() > 2 { parts[2] } else { "" };

        // Normalise status to "running", "exited", or "created"
        let status = if raw_status.starts_with("Up") {
            "running".to_string()
        } else if raw_status.starts_with("Exited") {
            "exited".to_string()
        } else if raw_status.starts_with("Created") {
            "created".to_string()
        } else {
            raw_status.to_lowercase()
        };

        // Extract host port from ports string like "0.0.0.0:5555->5555/tcp"
        let host_port = extract_host_port(ports_str).unwrap_or(0);

        vms.push(VmInfo {
            name,
            status,
            host_port,
            cpu_percent: String::new(),
            mem_usage: String::new(),
        });
    }

    Ok(vms)
}

/// Extract the host port from a Docker ports string like "0.0.0.0:5555->5555/tcp"
fn extract_host_port(ports: &str) -> Option<u16> {
    // Find the pattern ":PORT->" and extract PORT
    let colon_pos = ports.find(':')?;
    let arrow_pos = ports.find("->")?;
    if colon_pos >= arrow_pos {
        return None;
    }
    ports[colon_pos + 1..arrow_pos].parse::<u16>().ok()
}

pub fn create_vms(config: &HostConfig, count: usize) -> Result<Vec<String>, String> {
    // Scan existing VMs to find used ports and numbers
    let existing = list_vms(config).unwrap_or_default();

    let mut used_ports: Vec<u16> = existing.iter().map(|vm| vm.host_port).collect();
    let mut used_numbers: Vec<u32> = Vec::new();

    for vm in &existing {
        if let Some(n) = vm.name.strip_prefix("redroid-").and_then(|s| s.parse::<u32>().ok()) {
            used_numbers.push(n);
        }
    }

    let mut created_names = Vec::new();

    for _ in 0..count {
        // Find next available number
        let mut num = 1u32;
        while used_numbers.contains(&num) {
            num += 1;
        }
        used_numbers.push(num);

        // Find next available port
        let mut port = config.adb_port_start;
        while used_ports.contains(&port) && port <= config.adb_port_end {
            port += 1;
        }
        if port > config.adb_port_end {
            return Err(format!(
                "端口范围 {}-{} 已用完",
                config.adb_port_start, config.adb_port_end
            ));
        }
        used_ports.push(port);

        let container_name = format!("redroid-{num}");
        let cmd = format!(
            "docker run -d --name {name} --privileged \
             -p {port}:5555 \
             -v /root/redroid-data-{num}:/data \
             --memory=1536m --cpus=0.5 \
             {image} \
             androidboot.redroid_gpu_mode=host \
             androidboot.redroid_fps=30 \
             androidboot.use_memfd=1 \
             androidboot.redroid_width=720 \
             androidboot.redroid_height=1280 \
             ro.build.characteristics=default",
            name = container_name,
            port = port,
            num = num,
            image = config.image,
        );

        ssh_exec(config, &cmd)?;
        created_names.push(container_name);
    }

    Ok(created_names)
}

pub fn vm_action(config: &HostConfig, name: &str, action: &str) -> Result<String, String> {
    let docker_cmd = match action {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        _ => return Err(format!("未知操作: {action}")),
    };
    ssh_exec(config, &format!("docker {docker_cmd} {name}"))
}

pub fn delete_vm(config: &HostConfig, name: &str, purge_data: bool) -> Result<String, String> {
    let mut result = ssh_exec(config, &format!("docker rm -f {name}"))?;

    if purge_data {
        // Extract number from container name like "redroid-3"
        if let Some(num_str) = name.strip_prefix("redroid-") {
            if num_str.parse::<u32>().is_ok() {
                let purge_result =
                    ssh_exec(config, &format!("rm -rf /root/redroid-data-{num_str}"));
                match purge_result {
                    Ok(msg) => result.push_str(&msg),
                    Err(e) => return Err(format!("容器已删除但清理数据失败: {e}")),
                }
            }
        }
    }

    Ok(result)
}

pub fn batch_vm_action(
    config: &HostConfig,
    names: &[String],
    action: &str,
) -> Vec<(String, Result<String, String>)> {
    names
        .iter()
        .map(|name| {
            let result = vm_action(config, name, action);
            (name.clone(), result)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Anti-detection: property injection after boot
// ---------------------------------------------------------------------------

/// Inject anti-detection properties into a running container via resetprop.
/// Must be called after sys.boot_completed=1 and Magisk is active.
pub fn inject_anti_detection_props(config: &HostConfig, name: &str) -> Result<String, String> {
    let script = format!(
        r#"docker exec {name} sh -c '
/sbin/resetprop ro.board.platform kona
/sbin/resetprop ro.boot.hardware qcom
/sbin/resetprop ro.hardware.chipname kona
/sbin/resetprop ro.debuggable 0
/sbin/resetprop ro.secure 1
/sbin/resetprop ro.build.type user
/sbin/resetprop ro.build.fingerprint "Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys"
/sbin/resetprop ro.vendor.build.fingerprint "Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys"
/sbin/resetprop ro.system.build.fingerprint "Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys"
/sbin/resetprop --delete ro.lineage.device
/sbin/resetprop --delete ro.lineage.version
rm -f /.dockerenv
setprop gsm.network.type NR
setprop gsm.sim.state READY,READY
setprop gsm.sim.operator.alpha "中国移动"
setprop gsm.sim.operator.numeric 46000
setprop gsm.operator.alpha "中国移动"
setprop gsm.operator.numeric 46000
setprop persist.radio.multisim.config dsds
setprop gsm.version.baseband "4.0.2.c8-00039-0710_0125_f484f49"
setprop net.dns1 223.5.5.5
setprop net.dns2 223.6.6.6
echo PROVISION_OK
'"#,
        name = name
    );
    ssh_exec(config, &script)
}

/// Wait for container boot, then inject anti-detection props.
pub fn wait_and_provision(
    config: &HostConfig,
    name: &str,
    timeout_secs: u32,
) -> Result<String, String> {
    let wait_cmd = format!(
        r#"for i in $(seq 1 {timeout}); do
    BOOT=$(docker exec {name} getprop sys.boot_completed 2>/dev/null)
    [ "$BOOT" = "1" ] && break
    sleep 1
done
docker exec {name} getprop sys.boot_completed"#,
        timeout = timeout_secs,
        name = name,
    );

    let boot_status = ssh_exec(config, &wait_cmd)?;
    if boot_status.trim() != "1" {
        return Err(format!(
            "Container {name} did not boot within {timeout_secs}s"
        ));
    }

    inject_anti_detection_props(config, name)
}

/// One-click new device: destroy old, create fresh with new identity.
pub fn rotate_vm(config: &HostConfig, name: &str) -> Result<String, String> {
    let _ = delete_vm(config, name, true);

    let num_str = name.strip_prefix("redroid-").unwrap_or("1");
    let num: u32 = num_str.parse().unwrap_or(1);
    let port = config.adb_port_start + (num as u16) - 1;

    let cmd = format!(
        "docker run -d --name {name} --privileged \
         -p {port}:5555 \
         -v /root/redroid-data-{num}:/data \
         --memory=8g \
         {image} \
         androidboot.redroid_gpu_mode=mali \
         androidboot.redroid_gpu_node=/dev/mali0 \
         androidboot.redroid_fps=30 \
         androidboot.redroid_magisk=1 \
         ro.product.brand=Xiaomi \
         ro.product.model=M2001J2C \
         ro.product.device=umi \
         ro.product.manufacturer=Xiaomi \
         ro.build.tags=release-keys \
         ro.build.type=user \
         ro.debuggable=0",
        name = name,
        port = port,
        num = num,
        image = config.image,
    );

    ssh_exec(config, &cmd)?;
    wait_and_provision(config, name, 120)
}

/// Create VMs with Magisk and anti-detection enabled.
pub fn create_vms_v2(config: &HostConfig, count: usize) -> Result<Vec<String>, String> {
    let existing = list_vms(config).unwrap_or_default();
    let mut used_ports: Vec<u16> = existing.iter().map(|vm| vm.host_port).collect();
    let mut used_numbers: Vec<u32> = Vec::new();

    for vm in &existing {
        if let Some(n) = vm
            .name
            .strip_prefix("redroid-")
            .and_then(|s| s.parse::<u32>().ok())
        {
            used_numbers.push(n);
        }
    }

    let mut created_names = Vec::new();

    for _ in 0..count {
        let mut num = 1u32;
        while used_numbers.contains(&num) {
            num += 1;
        }
        used_numbers.push(num);

        let mut port = config.adb_port_start;
        while used_ports.contains(&port) && port <= config.adb_port_end {
            port += 1;
        }
        if port > config.adb_port_end {
            return Err(format!(
                "端口范围 {}-{} 已用完",
                config.adb_port_start, config.adb_port_end
            ));
        }
        used_ports.push(port);

        let container_name = format!("redroid-{num}");
        let cmd = format!(
            "docker run -d --name {name} --privileged \
             -p {port}:5555 \
             -v /root/redroid-data-{num}:/data \
             --memory=8g \
             {image} \
             androidboot.redroid_gpu_mode=mali \
             androidboot.redroid_gpu_node=/dev/mali0 \
             androidboot.redroid_fps=30 \
             androidboot.redroid_magisk=1 \
             ro.product.brand=Xiaomi \
             ro.product.model=M2001J2C \
             ro.product.device=umi \
             ro.product.manufacturer=Xiaomi \
             ro.build.tags=release-keys \
             ro.build.type=user \
             ro.debuggable=0",
            name = container_name,
            port = port,
            num = num,
            image = config.image,
        );

        ssh_exec(config, &cmd)?;
        created_names.push(container_name);
    }

    Ok(created_names)
}
