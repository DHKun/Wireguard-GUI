// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod wg;

use wg::conf::{PeerConf, WgConf};
use wg::ops;

/// 全部接口状态（含 Peer 明细、流量）。
#[tauri::command]
fn wg_status() -> Result<Vec<ops::InterfaceStatus>, String> {
    ops::linux().status_all()
}

/// /etc/wireguard 下的配置文件列表。
#[tauri::command]
fn list_configs() -> Result<Vec<String>, String> {
    ops::linux().list_configs()
}

/// 读取配置原文。
#[tauri::command]
fn read_config(name: String) -> Result<String, String> {
    ops::linux().read_config(&name)
}

/// 读取并解析配置。
#[tauri::command]
fn read_config_parsed(name: String) -> Result<WgConf, String> {
    ops::linux().read_config_parsed(&name)
}

/// 写回配置原文（0600，不自动应用）。
#[tauri::command]
fn write_config(name: String, content: String) -> Result<(), String> {
    ops::linux().write_config(&name, &content)
}

/// 写回结构化配置。
#[tauri::command]
fn write_config_parsed(name: String, conf: WgConf) -> Result<(), String> {
    ops::linux().write_config_parsed(&name, &conf)
}

/// Persist an Interface Configuration and optionally synchronize its Runtime Interface.
#[tauri::command]
fn apply_config(
    name: String,
    content: String,
    synchronize: bool,
) -> Result<ops::ApplyOutcome, String> {
    ops::linux().apply_config(&name, &content, synchronize)
}

/// 导出配置到目标路径（特权）。
#[tauri::command]
fn export_config(name: String, dest: String) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    ops::linux().export_config(&name, &dest, &home)
}

/// 启动接口（wg-quick up）。
#[tauri::command]
fn interface_up(name: String) -> Result<(), String> {
    ops::linux().interface_action(&name, ops::InterfaceAction::Up)
}

/// 停止接口（wg-quick down）。
#[tauri::command]
fn interface_down(name: String) -> Result<(), String> {
    ops::linux().interface_action(&name, ops::InterfaceAction::Down)
}

/// 重启接口。
#[tauri::command]
fn interface_restart(name: String) -> Result<(), String> {
    ops::linux().interface_action(&name, ops::InterfaceAction::Restart)
}

/// 热同步配置（wg-quick strip | wg syncconf），不中断接口。
#[tauri::command]
fn syncconf(name: String) -> Result<(), String> {
    ops::linux().interface_action(&name, ops::InterfaceAction::Sync)
}

/// Apply one lifecycle action to a Runtime Interface.
#[tauri::command]
fn interface_action(name: String, action: ops::InterfaceAction) -> Result<(), String> {
    ops::linux().interface_action(&name, action)
}

/// 对运行中的接口直接应用 Peer 集合（wg setconf）。
#[tauri::command]
fn set_peers(name: String, peers: Vec<PeerConf>) -> Result<(), String> {
    ops::linux().set_peers(&name, &peers)
}

/// Apply the complete Peer collection according to an Apply Mode.
#[tauri::command]
fn apply_peers(
    name: String,
    peers: Vec<PeerConf>,
    mode: ops::ApplyMode,
) -> Result<ops::ApplyOutcome, String> {
    ops::linux().apply_peers(&name, &peers, mode)
}

/// 生成新密钥对。
#[tauri::command]
fn generate_keypair() -> Result<ops::KeyPair, String> {
    ops::linux().generate_keypair()
}

/// 生成预共享密钥。
#[tauri::command]
fn generate_preshared_key() -> Result<String, String> {
    ops::linux().generate_preshared_key()
}

/// 由私钥推导公钥。
#[tauri::command]
fn derive_pubkey(private_key: String) -> Result<String, String> {
    ops::linux().derive_public_key(&private_key)
}

/// 检查 wg/wg-quick/pkexec 是否可用。
#[tauri::command]
fn check_env() -> Result<ops::EnvCheck, String> {
    Ok(ops::environment())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            wg_status,
            list_configs,
            read_config,
            read_config_parsed,
            write_config,
            write_config_parsed,
            apply_config,
            export_config,
            interface_up,
            interface_down,
            interface_restart,
            syncconf,
            interface_action,
            set_peers,
            apply_peers,
            generate_keypair,
            generate_preshared_key,
            derive_pubkey,
            check_env,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
