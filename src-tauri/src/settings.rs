//! App shell settings: autostart, silent start, and close-to-tray.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_DIR: &str = "wireguard-gui";
const SETTINGS_FILE: &str = "settings.json";
const AUTOSTART_FILE: &str = "wireguard-gui.desktop";
const SILENT_FLAG: &str = "--silent";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    pub autostart: bool,
    pub silent_start: bool,
    pub close_to_tray: bool,
}

pub fn wants_silent_start(args: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    args.into_iter()
        .any(|arg| matches!(arg.as_ref(), "--silent" | "-s"))
}

pub fn load_settings() -> AppSettings {
    load_settings_from(&settings_path())
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    save_settings_to(&settings_path(), settings)
}

pub fn apply_autostart(settings: &AppSettings) -> Result<(), String> {
    apply_autostart_at(&autostart_path(), &launch_command()?, settings)
}

fn xdg_config_home() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config")
}

fn settings_path() -> PathBuf {
    xdg_config_home().join(SETTINGS_DIR).join(SETTINGS_FILE)
}

fn autostart_path() -> PathBuf {
    xdg_config_home().join("autostart").join(AUTOSTART_FILE)
}

fn load_settings_from(path: &Path) -> AppSettings {
    let Ok(text) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_settings_to(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建设置目录失败: {error}"))?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("序列化设置失败: {error}"))?;
    fs::write(path, text).map_err(|error| format!("写入设置失败: {error}"))
}

fn apply_autostart_at(
    path: &Path,
    command: &str,
    settings: &AppSettings,
) -> Result<(), String> {
    if !settings.autostart {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("删除开机自启项失败: {error}")),
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建开机自启目录失败: {error}"))?;
        }
        fs::write(
            path,
            autostart_desktop_contents(&quote_command(command), settings.silent_start),
        )
        .map_err(|error| format!("写入开机自启项失败: {error}"))
    }
}

fn launch_command() -> Result<String, String> {
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        if !appimage.is_empty() {
            return Ok(appimage);
        }
    }
    let exe = std::env::current_exe().map_err(|error| format!("解析当前程序路径失败: {error}"))?;
    Ok(exe.display().to_string())
}

fn quote_command(path: &str) -> String {
    if path.chars().any(char::is_whitespace) {
        format!("\"{}\"", path.replace('"', "\\\""))
    } else {
        path.to_string()
    }
}

fn autostart_desktop_contents(command: &str, silent: bool) -> String {
    let exec = if silent {
        format!("{command} {SILENT_FLAG}")
    } else {
        command.to_string()
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=WireGuard 控制台\n\
         Comment=登录后启动 WireGuard 控制台\n\
         Exec={exec}\n\
         Icon=wireguard-gui\n\
         Terminal=false\n\
         Categories=Utility;Network;\n\
         X-GNOME-Autostart-enabled=true\n\
         StartupNotify=false\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_flag_is_detected() {
        assert!(wants_silent_start(["wireguard-gui", "--silent"]));
        assert!(wants_silent_start(["-s"]));
        assert!(!wants_silent_start(["wireguard-gui"]));
    }

    #[test]
    fn unknown_settings_fields_are_ignored() {
        let parsed: AppSettings =
            serde_json::from_str(r#"{"autostart":true,"extra":1}"#).unwrap();
        assert!(parsed.autostart);
        assert!(!parsed.silent_start);
        assert!(!parsed.close_to_tray);
    }

    #[test]
    fn desktop_file_includes_silent_flag_only_when_requested() {
        let silent = autostart_desktop_contents("/usr/bin/wireguard-gui", true);
        let visible = autostart_desktop_contents("/usr/bin/wireguard-gui", false);
        assert!(silent.contains("Exec=/usr/bin/wireguard-gui --silent\n"));
        assert!(visible.contains("Exec=/usr/bin/wireguard-gui\n"));
        assert!(!visible.contains("--silent"));
    }

    #[test]
    fn apply_autostart_writes_and_removes_desktop_entry() {
        let root = std::env::temp_dir().join(format!(
            "wireguard-gui-autostart-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("autostart").join(AUTOSTART_FILE);
        let enabled = AppSettings {
            autostart: true,
            silent_start: true,
            close_to_tray: true,
        };

        apply_autostart_at(&path, "/opt/WireGuard GUI/wireguard-gui", &enabled).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Exec=\"/opt/WireGuard GUI/wireguard-gui\" --silent\n"));

        apply_autostart_at(&path, "/opt/app", &AppSettings::default()).unwrap();
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn settings_roundtrip_preserves_values() {
        let root = std::env::temp_dir().join(format!(
            "wireguard-gui-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join(SETTINGS_FILE);
        let settings = AppSettings {
            autostart: true,
            silent_start: true,
            close_to_tray: true,
        };
        save_settings_to(&path, &settings).unwrap();
        assert_eq!(load_settings_from(&path), settings);
        fs::remove_dir_all(root).unwrap();
    }
}
