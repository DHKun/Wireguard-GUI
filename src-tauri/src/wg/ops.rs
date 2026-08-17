//! 高层操作：把 dump/conf 解析与 pkexec 桥组合成完整功能。

use serde::Serialize;

use super::conf::{parse_conf, serialize_conf, serialize_peers_for_setconf, WgConf};
use super::dump::parse_dump;
use super::elevate::{pkexec, read_file, wg, wg_quick, write_file};

pub const CONF_DIR: &str = "/etc/wireguard";
pub const WG_BIN: &str = "/usr/bin/wg";
pub const WG_QUICK_BIN: &str = "/usr/bin/wg-quick";

/// 校验配置文件名：仅允许常规 .conf 名，杜绝路径穿越。
pub fn validate_conf_name(name: &str) -> Result<(), String> {
    let ok = name.len() <= 64
        && name.ends_with(".conf")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !name.starts_with('.');
    if ok {
        Ok(())
    } else {
        Err(format!("非法配置文件名: {name:?}"))
    }
}

/// 校验接口名（Linux 接口名 ≤15 字符）。
pub fn validate_iface_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 15
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(format!("非法接口名: {name:?}"))
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct InterfaceStatus {
    pub name: String,
    pub running: bool,
    pub public_key: String,
    pub private_key: Option<String>,
    pub listen_port: u16,
    pub fwmark: Option<String>,
    pub addresses: Vec<String>,
    pub mtu: Option<u32>,
    /// 接口总流量（/sys/class/net 统计，免特权）
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub peers: Vec<super::dump::PeerStatus>,
}

fn sysfs_u64(path: &str) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// 经 `ip -j addr`（免特权）取接口地址与 MTU。
fn iface_addrs(name: &str) -> (Vec<String>, Option<u32>) {
    let out = std::process::Command::new("/usr/bin/ip")
        .args(["-j", "addr", "show", "dev", name])
        .output();
    let mut addrs = Vec::new();
    let mut mtu = None;
    if let Ok(out) = out {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json.as_array() {
                if let Some(entry) = arr.first() {
                    mtu = entry.get("mtu").and_then(|v| v.as_u64()).map(|v| v as u32);
                    if let Some(list) = entry.get("addr_info").and_then(|v| v.as_array()) {
                        for a in list {
                            if let (Some(local), Some(prefix)) = (
                                a.get("local").and_then(|v| v.as_str()),
                                a.get("prefixlen").and_then(|v| v.as_u64()),
                            ) {
                                addrs.push(format!("{local}/{prefix}"));
                            }
                        }
                    }
                }
            }
        }
    }
    (addrs, mtu)
}

/// 读取全部接口状态（dump 走 pkexec，流量走 sysfs）。
/// 已停止的接口（存在 .conf 但未运行）也列出，便于从 GUI 启动。
pub fn status_all() -> Result<Vec<InterfaceStatus>, String> {
    let dump = String::from_utf8_lossy(&wg(&["show", "all", "dump"], None)?).into_owned();
    let parsed = parse_dump(&dump)?;
    let mut out = Vec::with_capacity(parsed.len());
    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    for iface in parsed {
        let (addrs, mtu) = iface_addrs(&iface.name);
        let rx = sysfs_u64(&format!("/sys/class/net/{}/statistics/rx_bytes", iface.name));
        let tx = sysfs_u64(&format!("/sys/class/net/{}/statistics/tx_bytes", iface.name));
        known.insert(iface.name.clone());
        out.push(InterfaceStatus {
            addresses: addrs,
            mtu,
            rx_bytes: rx,
            tx_bytes: tx,
            running: true,
            name: iface.name,
            public_key: iface.public_key,
            private_key: iface.private_key,
            listen_port: iface.listen_port,
            fwmark: iface.fwmark,
            peers: iface.peers,
        });
    }
    // 合并已停止但存在配置文件的接口
    if let Ok(configs) = list_configs() {
        for c in configs {
            let name = c.trim_end_matches(".conf").to_string();
            if known.insert(name.clone()) {
                out.push(InterfaceStatus {
                    name,
                    running: false,
                    public_key: String::new(),
                    private_key: None,
                    listen_port: 0,
                    fwmark: None,
                    addresses: Vec::new(),
                    mtu: None,
                    rx_bytes: 0,
                    tx_bytes: 0,
                    peers: Vec::new(),
                });
            }
        }
    }
    Ok(out)
}

/// 列出 /etc/wireguard 下的 .conf 文件。
pub fn list_configs() -> Result<Vec<String>, String> {
    let out = pkexec(&["/usr/bin/ls", "-1", CONF_DIR], None)?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| l.ends_with(".conf") && !l.is_empty())
        .collect())
}

/// 读取配置原文（特权）。
pub fn read_config(name: &str) -> Result<String, String> {
    validate_conf_name(name)?;
    read_file(&format!("{CONF_DIR}/{name}"))
}

/// 写入配置（特权 tee + 0600），不自动应用。
pub fn write_config(name: &str, content: &str) -> Result<(), String> {
    validate_conf_name(name)?;
    let path = format!("{CONF_DIR}/{name}");
    write_file(&path, content, Some("600"))?;
    Ok(())
}

/// 导出配置到用户指定路径（特权 install，0600，自动建父目录）。
pub fn export_config(name: &str, dest: &str) -> Result<(), String> {
    validate_conf_name(name)?;
    if !dest.starts_with('/') {
        return Err("目标路径必须是绝对路径".to_string());
    }
    let src = format!("{CONF_DIR}/{name}");
    pkexec(&["/usr/bin/install", "-m", "600", "-D", &src, dest], None).map(|_| ())
}

/// 读取配置并解析为结构化模型（特权读）。
pub fn read_config_parsed(name: &str) -> Result<WgConf, String> {
    parse_conf(&read_config(name)?)
}

/// 把结构化模型写回文件（特权）。
pub fn write_config_parsed(name: &str, conf: &WgConf) -> Result<(), String> {
    write_config(name, &serialize_conf(conf))
}

/// 启动接口：wg-quick up。
pub fn interface_up(name: &str) -> Result<(), String> {
    validate_iface_name(name)?;
    wg_quick(&["up", name])
}

/// 停止接口：wg-quick down。
pub fn interface_down(name: &str) -> Result<(), String> {
    validate_iface_name(name)?;
    wg_quick(&["down", name])
}

/// 重启接口。
pub fn interface_restart(name: &str) -> Result<(), String> {
    validate_iface_name(name)?;
    wg_quick(&["down", name])?;
    wg_quick(&["up", name])
}

/// 热同步：wg-quick strip | wg syncconf，不中断接口。
pub fn syncconf(name: &str) -> Result<(), String> {
    validate_iface_name(name)?;
    validate_conf_name(&format!("{name}.conf"))?;
    if !std::path::Path::new(&format!("/sys/class/net/{name}")).exists() {
        return Err(format!("接口 {name} 未运行，无法热同步（配置已保存）"));
    }
    // wg syncconf 从 stdin 读配置
    let conf = read_config(&format!("{name}.conf"))?;
    pkexec(&[WG_BIN, "syncconf", name, "/dev/stdin"], Some(conf.as_bytes())).map(|_| ())
}

/// 直接对运行中的接口应用 Peer 集合（wg setconf，密钥走 stdin）。
pub fn set_peers(name: &str, peers: &[super::conf::PeerConf]) -> Result<(), String> {
    validate_iface_name(name)?;
    let text = serialize_peers_for_setconf(peers);
    wg(&["setconf", name, "/dev/stdin"], Some(text.as_bytes())).map(|_| ())
}

/// 生成密钥对（wg genkey + wg pubkey，均免特权，密钥不进 argv）。
#[derive(Serialize)]
pub struct KeyPair {
    pub private_key: String,
    pub public_key: String,
}

pub fn generate_keypair() -> Result<KeyPair, String> {
    let priv_out = std::process::Command::new(WG_BIN)
        .arg("genkey")
        .output()
        .map_err(|e| format!("wg genkey 失败: {e}"))?;
    if !priv_out.status.success() {
        return Err(format!(
            "wg genkey 失败: {}",
            String::from_utf8_lossy(&priv_out.stderr).trim()
        ));
    }
    let private_key = String::from_utf8_lossy(&priv_out.stdout).trim().to_string();

    // 公钥从 stdin 计算，避免私钥出现在进程参数里
    let mut child = std::process::Command::new(WG_BIN)
        .arg("pubkey")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("wg pubkey 启动失败: {e}"))?;
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(private_key.as_bytes())
        .map_err(|e| format!("写入 wg pubkey 失败: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("wg pubkey 失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "wg pubkey 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let public_key = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(KeyPair {
        private_key,
        public_key,
    })
}

/// 生成预共享密钥（wg genpsk，免特权）。
pub fn generate_preshared_key() -> Result<String, String> {
    let out = std::process::Command::new(WG_BIN)
        .arg("genpsk")
        .output()
        .map_err(|e| format!("wg genpsk 失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "wg genpsk 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 由私钥推导公钥（stdin 传递）。
pub fn derive_pubkey(private_key: &str) -> Result<String, String> {
    let mut child = std::process::Command::new(WG_BIN)
        .arg("pubkey")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("wg pubkey 启动失败: {e}"))?;
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(private_key.as_bytes())
        .map_err(|e| format!("写入失败: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("wg pubkey 失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "wg pubkey 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
