// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod settings;
mod wg;

use settings::AppSettings;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};
use wg::conf::PeerConf;
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

/// Apply one lifecycle action to a Runtime Interface.
#[tauri::command]
fn interface_action(name: String, action: ops::InterfaceAction) -> Result<(), String> {
    ops::linux().interface_action(&name, action)
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

/// 检查 wg/wg-quick/pkexec 是否可用。
#[tauri::command]
fn check_env() -> Result<ops::EnvCheck, String> {
    Ok(ops::environment())
}

#[tauri::command]
fn get_app_settings(state: tauri::State<'_, Mutex<AppSettings>>) -> AppSettings {
    state.lock().expect("settings lock").clone()
}

#[tauri::command]
fn update_app_settings(
    next: AppSettings,
    state: tauri::State<'_, Mutex<AppSettings>>,
) -> Result<AppSettings, String> {
    settings::save_settings(&next)?;
    settings::apply_autostart(&next)?;
    *state.lock().expect("settings lock") = next.clone();
    Ok(next)
}

/// WebKitGTK 的 DMABUF 渲染器在 Wayland（尤其 KDE + 部分 GPU 驱动）上会触发
/// Wayland 协议错误（Gdk-Message: Error 71）导致窗口创建即崩溃闪退。
/// 启动时若处于 Wayland 且用户未显式设置该变量，则默认禁用 DMABUF 渲染。
/// 用户可通过 WEBKIT_DISABLE_DMABUF_RENDERER=0 显式恢复硬件加速路径。
#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("缺少应用图标，无法创建托盘")?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("WireGuard 控制台")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();

    let loaded = settings::load_settings();
    // 仅命令行 --silent 才隐藏窗口。设置项 silent_start 只写进开机自启的 Exec。
    let start_hidden = settings::wants_silent_start(std::env::args());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(loaded))
        .invoke_handler(tauri::generate_handler![
            wg_status,
            list_configs,
            read_config,
            apply_config,
            export_config,
            interface_action,
            apply_peers,
            generate_keypair,
            generate_preshared_key,
            check_env,
            get_app_settings,
            update_app_settings,
        ])
        .setup(move |app| {
            setup_tray(app)?;
            if start_hidden {
                hide_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .state::<Mutex<AppSettings>>()
                    .lock()
                    .map(|settings| settings.close_to_tray)
                    .unwrap_or(false);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.app_handle().emit("app://minimized-to-tray", ());
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
