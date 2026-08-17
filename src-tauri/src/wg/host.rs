//! Linux host adapter: owns process execution, privilege elevation, paths, and file permissions.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

pub const CONF_DIR: &str = "/etc/wireguard";
pub const WG_BIN: &str = "/usr/bin/wg";
pub const WG_QUICK_BIN: &str = "/usr/bin/wg-quick";
pub const PKEXEC_BIN: &str = "/usr/bin/pkexec";
const IP_BIN: &str = "/usr/bin/ip";
const INSTALL_BIN: &str = "/usr/bin/install";
const CAT_BIN: &str = "/usr/bin/cat";
const LS_BIN: &str = "/usr/bin/ls";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceAction {
    Up,
    Down,
    Restart,
    Sync,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InterfaceDetails {
    pub addresses: Vec<String>,
    pub mtu: Option<u32>,
}

/// Internal seam for process execution. Production and recording test adapters share it.
pub(crate) trait CommandRunner {
    fn run(
        &self,
        privileged: bool,
        program: &str,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<Vec<u8>, String>;
}

#[derive(Default)]
pub(crate) struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(
        &self,
        privileged: bool,
        program: &str,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let mut command = if privileged {
            let mut command = Command::new(PKEXEC_BIN);
            command.arg(program);
            command
        } else {
            Command::new(program)
        };
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| format!("无法启动 {program}: {error}"))?;
        if let Some(data) = stdin {
            child
                .stdin
                .take()
                .expect("stdin piped")
                .write_all(data)
                .map_err(|error| format!("写入 {program} stdin 失败: {error}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("等待 {program} 失败: {error}"))?;
        if output.status.success() {
            return Ok(output.stdout);
        }

        let code = output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".into());
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if error.is_empty() {
            Err(format!("{program} 失败（exit {code}）"))
        } else {
            Err(format!("{program} 失败（exit {code}）: {error}"))
        }
    }
}

/// Host interface used by the deep WireGuard module and its in-memory test adapter.
pub trait WireGuardHost {
    fn show_all_dump(&self) -> Result<String, String>;
    fn list_config_names(&self) -> Result<Vec<String>, String>;
    fn read_config(&self, name: &str) -> Result<String, String>;
    fn write_config(&self, name: &str, content: &str) -> Result<(), String>;
    fn export_config(&self, name: &str, destination: &Path, home: &Path) -> Result<(), String>;
    fn interface_details(&self, name: &str) -> InterfaceDetails;
    fn interface_stats(&self, name: &str) -> (u64, u64);
    fn interface_exists(&self, name: &str) -> bool;
    fn apply_interface(&self, name: &str, action: InterfaceAction) -> Result<(), String>;
    fn set_peers(&self, name: &str, configuration: &str) -> Result<(), String>;
    fn generate_private_key(&self) -> Result<String, String>;
    fn derive_public_key(&self, private_key: &str) -> Result<String, String>;
    fn generate_preshared_key(&self) -> Result<String, String>;
}

pub struct LinuxHost<R = ProcessRunner> {
    runner: R,
}

impl Default for LinuxHost<ProcessRunner> {
    fn default() -> Self {
        Self {
            runner: ProcessRunner,
        }
    }
}

impl<R> LinuxHost<R> {
    #[cfg(test)]
    fn with_runner(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: CommandRunner> LinuxHost<R> {
    fn run_privileged(
        &self,
        program: &str,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.runner.run(true, program, args, stdin)
    }

    fn run_user(
        &self,
        program: &str,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.runner.run(false, program, args, stdin)
    }

    fn config_path(name: &str) -> String {
        format!("{CONF_DIR}/{name}")
    }

    fn sysfs_u64(path: &str) -> u64 {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0)
    }

    fn parse_interface_details(output: &[u8]) -> InterfaceDetails {
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(output) else {
            return InterfaceDetails::default();
        };
        let Some(entry) = json.as_array().and_then(|entries| entries.first()) else {
            return InterfaceDetails::default();
        };

        let mtu = entry
            .get("mtu")
            .and_then(|value| value.as_u64())
            .map(|value| value as u32);
        let addresses = entry
            .get("addr_info")
            .and_then(|value| value.as_array())
            .map(|addresses| {
                addresses
                    .iter()
                    .filter_map(|address| {
                        let local = address.get("local")?.as_str()?;
                        let prefix = address.get("prefixlen")?.as_u64()?;
                        Some(format!("{local}/{prefix}"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        InterfaceDetails { addresses, mtu }
    }
}

impl<R: CommandRunner> WireGuardHost for LinuxHost<R> {
    fn show_all_dump(&self) -> Result<String, String> {
        self.run_privileged(WG_BIN, &["show", "all", "dump"], None)
            .map(|output| String::from_utf8_lossy(&output).into_owned())
    }

    fn list_config_names(&self) -> Result<Vec<String>, String> {
        let output = self.run_privileged(LS_BIN, &["-1", CONF_DIR], None)?;
        Ok(String::from_utf8_lossy(&output)
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect())
    }

    fn read_config(&self, name: &str) -> Result<String, String> {
        let path = Self::config_path(name);
        self.run_privileged(CAT_BIN, &[&path], None)
            .map(|output| String::from_utf8_lossy(&output).into_owned())
    }

    fn write_config(&self, name: &str, content: &str) -> Result<(), String> {
        let path = Self::config_path(name);
        self.run_privileged(
            INSTALL_BIN,
            &["-m", "600", "-D", "/dev/stdin", &path],
            Some(content.as_bytes()),
        )
        .map(|_| ())
    }

    fn export_config(&self, name: &str, destination: &Path, home: &Path) -> Result<(), String> {
        let content = self.read_config(name)?;
        let destination = secure_export_destination(destination, home)?;
        if fs::symlink_metadata(&destination)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err("拒绝覆盖符号链接".to_string());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(destination)
            .map_err(|error| format!("创建导出文件失败: {error}"))?;
        file.write_all(content.as_bytes())
            .map_err(|error| format!("写入导出文件失败: {error}"))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("设置导出文件权限失败: {error}"))
    }

    fn interface_details(&self, name: &str) -> InterfaceDetails {
        self.run_user(IP_BIN, &["-j", "addr", "show", "dev", name], None)
            .map(|output| Self::parse_interface_details(&output))
            .unwrap_or_default()
    }

    fn interface_stats(&self, name: &str) -> (u64, u64) {
        let root = format!("/sys/class/net/{name}/statistics");
        (
            Self::sysfs_u64(&format!("{root}/rx_bytes")),
            Self::sysfs_u64(&format!("{root}/tx_bytes")),
        )
    }

    fn interface_exists(&self, name: &str) -> bool {
        Path::new(&format!("/sys/class/net/{name}")).exists()
    }

    fn apply_interface(&self, name: &str, action: InterfaceAction) -> Result<(), String> {
        match action {
            InterfaceAction::Up => self
                .run_privileged(WG_QUICK_BIN, &["up", name], None)
                .map(|_| ()),
            InterfaceAction::Down => self
                .run_privileged(WG_QUICK_BIN, &["down", name], None)
                .map(|_| ()),
            InterfaceAction::Restart => {
                self.run_privileged(WG_QUICK_BIN, &["down", name], None)?;
                self.run_privileged(WG_QUICK_BIN, &["up", name], None)
                    .map(|_| ())
            }
            InterfaceAction::Sync => {
                let stripped = self.run_privileged(WG_QUICK_BIN, &["strip", name], None)?;
                self.run_privileged(WG_BIN, &["syncconf", name, "/dev/stdin"], Some(&stripped))
                    .map(|_| ())
            }
        }
    }

    fn set_peers(&self, name: &str, configuration: &str) -> Result<(), String> {
        self.run_privileged(
            WG_BIN,
            &["setconf", name, "/dev/stdin"],
            Some(configuration.as_bytes()),
        )
        .map(|_| ())
    }

    fn generate_private_key(&self) -> Result<String, String> {
        self.run_user(WG_BIN, &["genkey"], None)
            .map(|output| String::from_utf8_lossy(&output).trim().to_string())
    }

    fn derive_public_key(&self, private_key: &str) -> Result<String, String> {
        self.run_user(WG_BIN, &["pubkey"], Some(private_key.as_bytes()))
            .map(|output| String::from_utf8_lossy(&output).trim().to_string())
    }

    fn generate_preshared_key(&self) -> Result<String, String> {
        self.run_user(WG_BIN, &["genpsk"], None)
            .map(|output| String::from_utf8_lossy(&output).trim().to_string())
    }
}

fn secure_export_destination(
    destination: &Path,
    home: &Path,
) -> Result<std::path::PathBuf, String> {
    let relative = destination
        .strip_prefix(home)
        .map_err(|_| "导出路径必须位于当前用户主目录内".to_string())?;
    let file_name = relative
        .file_name()
        .ok_or_else(|| "导出路径缺少文件名".to_string())?;
    let canonical_home = home
        .canonicalize()
        .map_err(|error| format!("解析用户主目录失败: {error}"))?;
    let mut parent = canonical_home;
    if let Some(relative_parent) = relative.parent() {
        for component in relative_parent.components() {
            let std::path::Component::Normal(part) = component else {
                return Err("导出路径包含非法目录".to_string());
            };
            parent.push(part);
            match fs::symlink_metadata(&parent) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err("导出路径包含符号链接目录".to_string());
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err("导出路径包含非目录项".to_string());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&parent)
                        .map_err(|error| format!("创建导出目录失败: {error}"))?;
                }
                Err(error) => return Err(format!("检查导出目录失败: {error}")),
            }
        }
    }
    Ok(parent.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::os::unix::fs::symlink;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Call {
        privileged: bool,
        program: String,
        args: Vec<String>,
        stdin: Option<Vec<u8>>,
    }

    #[derive(Default)]
    struct RecordingRunner {
        calls: RefCell<Vec<Call>>,
        outputs: RefCell<VecDeque<Result<Vec<u8>, String>>>,
    }

    impl RecordingRunner {
        fn with_outputs(outputs: Vec<Result<Vec<u8>, String>>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                outputs: RefCell::new(outputs.into()),
            }
        }
    }

    impl CommandRunner for RecordingRunner {
        fn run(
            &self,
            privileged: bool,
            program: &str,
            args: &[&str],
            stdin: Option<&[u8]>,
        ) -> Result<Vec<u8>, String> {
            self.calls.borrow_mut().push(Call {
                privileged,
                program: program.to_string(),
                args: args.iter().map(|arg| (*arg).to_string()).collect(),
                stdin: stdin.map(|value| value.to_vec()),
            });
            self.outputs
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(Vec::new()))
        }
    }

    #[test]
    fn sync_strips_wg_quick_configuration_before_syncconf() {
        let runner = RecordingRunner::with_outputs(vec![
            Ok(b"[Interface]\nListenPort = 51820\n".to_vec()),
            Ok(Vec::new()),
        ]);
        let host = LinuxHost::with_runner(runner);

        host.apply_interface("wg0", InterfaceAction::Sync).unwrap();

        let calls = host.runner.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].program, WG_QUICK_BIN);
        assert_eq!(calls[0].args, ["strip".to_string(), "wg0".to_string()]);
        assert_eq!(calls[1].program, WG_BIN);
        assert_eq!(
            calls[1].args,
            [
                "syncconf".to_string(),
                "wg0".to_string(),
                "/dev/stdin".to_string(),
            ]
        );
        assert_eq!(
            calls[1].stdin.as_deref(),
            Some(b"[Interface]\nListenPort = 51820\n".as_slice())
        );
    }

    #[test]
    fn configuration_write_is_one_privileged_install_with_stdin() {
        let host = LinuxHost::with_runner(RecordingRunner::default());

        host.write_config("wg0.conf", "secret-config").unwrap();

        let calls = host.runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].privileged);
        assert_eq!(calls[0].program, INSTALL_BIN);
        assert_eq!(
            calls[0].args,
            [
                "-m".to_string(),
                "600".to_string(),
                "-D".to_string(),
                "/dev/stdin".to_string(),
                "/etc/wireguard/wg0.conf".to_string(),
            ]
        );
        assert_eq!(calls[0].stdin.as_deref(), Some(b"secret-config".as_slice()));
    }

    #[test]
    fn system_install_accepts_dev_stdin_as_the_source() {
        let root = std::env::temp_dir().join(format!(
            "wireguard-gui-install-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let destination = root.join("wg0.conf");
        let destination_text = destination.to_string_lossy().into_owned();

        ProcessRunner
            .run(
                false,
                INSTALL_BIN,
                &["-m", "600", "-D", "/dev/stdin", &destination_text],
                Some(b"secret-config"),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "secret-config");
        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_key_only_crosses_the_runner_via_stdin() {
        let runner = RecordingRunner::with_outputs(vec![Ok(b"public-key\n".to_vec())]);
        let host = LinuxHost::with_runner(runner);

        let public_key = host.derive_public_key("private-key").unwrap();

        assert_eq!(public_key, "public-key");
        let calls = host.runner.calls.borrow();
        assert_eq!(calls[0].args, ["pubkey".to_string()]);
        assert!(!calls[0].args.iter().any(|arg| arg.contains("private-key")));
        assert_eq!(calls[0].stdin.as_deref(), Some(b"private-key".as_slice()));
    }

    #[test]
    fn export_rejects_symlinked_parent_directory() {
        let root = std::env::temp_dir().join(format!(
            "wireguard-gui-export-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = root.join("home");
        let outside = root.join("outside");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, home.join("escape")).unwrap();

        let result = secure_export_destination(&home.join("escape/wg0.conf"), &home);

        assert!(result.is_err());
        fs::remove_dir_all(&root).unwrap();
    }
}
