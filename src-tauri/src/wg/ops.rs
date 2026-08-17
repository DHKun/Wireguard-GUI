//! Deep WireGuard module: owns validation, configuration lifecycle, runtime application, and outcomes.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use super::conf::{parse_conf, serialize_conf, serialize_peers_for_setconf, PeerConf, WgConf};
use super::dump::parse_dump;
pub use super::host::{
    InterfaceAction, LinuxHost, WireGuardHost, CONF_DIR, PKEXEC_BIN, WG_BIN, WG_QUICK_BIN,
};

/// Configuration names are basenames under /etc/wireguard.
pub fn validate_conf_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name.ends_with(".conf")
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
        && !name.starts_with('.');
    if valid {
        Ok(())
    } else {
        Err(format!("非法配置文件名: {name:?}"))
    }
}

/// Linux interface names are at most 15 bytes and use a conservative safe character set.
pub fn validate_iface_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.len() <= 15
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        });
    if valid {
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
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub peers: Vec<super::dump::PeerStatus>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub persisted: bool,
    pub runtime_applied: bool,
    pub warnings: Vec<String>,
}

impl ApplyOutcome {
    fn persisted() -> Self {
        Self {
            persisted: true,
            runtime_applied: false,
            warnings: Vec::new(),
        }
    }

    fn runtime() -> Self {
        Self {
            persisted: false,
            runtime_applied: true,
            warnings: Vec::new(),
        }
    }

    fn persisted_and_runtime() -> Self {
        Self {
            persisted: true,
            runtime_applied: true,
            warnings: Vec::new(),
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    RuntimeOnly,
    PersistAndSync,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct KeyPair {
    pub private_key: String,
    pub public_key: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct EnvCheck {
    pub wg: bool,
    pub wg_quick: bool,
    pub pkexec: bool,
    pub conf_dir_exists: bool,
    pub home: String,
}

/// Deep module. Callers express WireGuard intent; the host adapter owns Linux mechanics.
pub struct WireGuard<H> {
    host: H,
}

impl<H> WireGuard<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

pub fn linux() -> WireGuard<LinuxHost> {
    WireGuard::new(LinuxHost::default())
}

impl<H: WireGuardHost> WireGuard<H> {
    pub fn status_all(&self) -> Result<Vec<InterfaceStatus>, String> {
        let parsed = parse_dump(&self.host.show_all_dump()?)?;
        let mut statuses = Vec::with_capacity(parsed.len());
        let mut known = HashSet::new();

        for interface in parsed {
            validate_iface_name(&interface.name)?;
            let details = self.host.interface_details(&interface.name);
            let (rx_bytes, tx_bytes) = self.host.interface_stats(&interface.name);
            known.insert(interface.name.clone());
            statuses.push(InterfaceStatus {
                name: interface.name,
                running: true,
                public_key: interface.public_key,
                private_key: interface.private_key,
                listen_port: interface.listen_port,
                fwmark: interface.fwmark,
                addresses: details.addresses,
                mtu: details.mtu,
                rx_bytes,
                tx_bytes,
                peers: interface.peers,
            });
        }

        if let Ok(configurations) = self.list_configs() {
            for configuration in configurations {
                let name = configuration.trim_end_matches(".conf").to_string();
                if known.insert(name.clone()) {
                    statuses.push(InterfaceStatus {
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
        Ok(statuses)
    }

    pub fn list_configs(&self) -> Result<Vec<String>, String> {
        let mut names: Vec<String> = self
            .host
            .list_config_names()?
            .into_iter()
            .filter(|name| validate_conf_name(name).is_ok())
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn read_config(&self, name: &str) -> Result<String, String> {
        validate_conf_name(name)?;
        self.host.read_config(name)
    }

    fn read_config_parsed(&self, name: &str) -> Result<WgConf, String> {
        parse_conf(&self.read_config(name)?)
    }

    fn write_config(&self, name: &str, content: &str) -> Result<(), String> {
        validate_conf_name(name)?;
        self.host.write_config(name, content)
    }

    fn write_config_parsed(&self, name: &str, configuration: &WgConf) -> Result<(), String> {
        self.write_config(name, &serialize_conf(configuration))
    }

    /// Persist an Interface Configuration and optionally synchronize its Runtime Interface.
    /// Persistence success is retained in the outcome when synchronization fails.
    pub fn apply_config(
        &self,
        name: &str,
        content: &str,
        synchronize: bool,
    ) -> Result<ApplyOutcome, String> {
        parse_conf(content)?;
        self.write_config(name, content)?;
        if !synchronize {
            return Ok(ApplyOutcome::persisted());
        }

        let interface = name.trim_end_matches(".conf");
        if !self.host.interface_exists(interface) {
            return Ok(ApplyOutcome {
                persisted: true,
                runtime_applied: false,
                warnings: vec![format!("接口 {interface} 未运行，配置已保存")],
            });
        }
        match self.host.apply_interface(interface, InterfaceAction::Sync) {
            Ok(()) => Ok(ApplyOutcome::persisted_and_runtime()),
            Err(error) => Ok(ApplyOutcome {
                persisted: true,
                runtime_applied: false,
                warnings: vec![format!("配置已保存，热同步失败: {error}")],
            }),
        }
    }

    pub fn export_config(&self, name: &str, destination: &str, home: &str) -> Result<(), String> {
        validate_conf_name(name)?;
        let destination = validate_export_path(destination, home)?;
        self.host.export_config(name, &destination, Path::new(home))
    }

    pub fn interface_action(&self, name: &str, action: InterfaceAction) -> Result<(), String> {
        validate_iface_name(name)?;
        if action == InterfaceAction::Sync && !self.host.interface_exists(name) {
            return Err(format!("接口 {name} 未运行，无法热同步"));
        }
        self.host.apply_interface(name, action)
    }

    /// Apply the full Peer collection according to the requested Apply Mode.
    pub fn apply_peers(
        &self,
        name: &str,
        peers: &[PeerConf],
        mode: ApplyMode,
    ) -> Result<ApplyOutcome, String> {
        validate_iface_name(name)?;
        if mode == ApplyMode::RuntimeOnly {
            self.set_peers(name, peers)?;
            return Ok(ApplyOutcome::runtime());
        }

        let configuration_name = format!("{name}.conf");
        validate_conf_name(&configuration_name)?;
        if !self.list_configs()?.contains(&configuration_name) {
            self.set_peers(name, peers)?;
            return Ok(ApplyOutcome {
                persisted: false,
                runtime_applied: true,
                warnings: vec![format!(
                    "未找到 {configuration_name}，Peer 更改仅应用到 Runtime Interface"
                )],
            });
        }

        let mut configuration = self.read_config_parsed(&configuration_name)?;
        let existing_extras: HashMap<String, Vec<(String, String)>> = configuration
            .peers
            .iter()
            .map(|peer| (peer.public_key.clone(), peer.extras.clone()))
            .collect();
        let mut next_peers = peers.to_vec();
        for peer in &mut next_peers {
            if peer.extras.is_empty() {
                if let Some(extras) = existing_extras.get(&peer.public_key) {
                    peer.extras = extras.clone();
                }
            }
        }
        configuration.peers = next_peers;
        self.write_config_parsed(&configuration_name, &configuration)?;

        if !self.host.interface_exists(name) {
            return Ok(ApplyOutcome {
                persisted: true,
                runtime_applied: false,
                warnings: vec![format!("接口 {name} 未运行，Peer 更改已持久化")],
            });
        }
        match self.host.apply_interface(name, InterfaceAction::Sync) {
            Ok(()) => Ok(ApplyOutcome::persisted_and_runtime()),
            Err(error) => Ok(ApplyOutcome {
                persisted: true,
                runtime_applied: false,
                warnings: vec![format!("Peer 更改已持久化，热同步失败: {error}")],
            }),
        }
    }

    fn set_peers(&self, name: &str, peers: &[PeerConf]) -> Result<(), String> {
        validate_iface_name(name)?;
        self.host
            .set_peers(name, &serialize_peers_for_setconf(peers))
    }

    pub fn generate_keypair(&self) -> Result<KeyPair, String> {
        let private_key = self.host.generate_private_key()?;
        let public_key = self.host.derive_public_key(&private_key)?;
        Ok(KeyPair {
            private_key,
            public_key,
        })
    }

    pub fn generate_preshared_key(&self) -> Result<String, String> {
        self.host.generate_preshared_key()
    }
}

pub fn environment() -> EnvCheck {
    EnvCheck {
        wg: Path::new(WG_BIN).exists(),
        wg_quick: Path::new(WG_QUICK_BIN).exists(),
        pkexec: Path::new(PKEXEC_BIN).exists(),
        conf_dir_exists: Path::new(CONF_DIR).exists(),
        home: std::env::var("HOME").unwrap_or_default(),
    }
}

fn validate_export_path(destination: &str, home: &str) -> Result<PathBuf, String> {
    let destination = Path::new(destination);
    let home = Path::new(home);
    let has_parent_traversal = destination
        .components()
        .any(|component| component == Component::ParentDir);
    let valid = destination.is_absolute()
        && home.is_absolute()
        && destination.starts_with(home)
        && destination != home
        && destination.file_name().is_some()
        && !has_parent_traversal;
    if valid {
        Ok(destination.to_path_buf())
    } else {
        Err("导出路径必须位于当前用户主目录内".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wg::host::InterfaceDetails;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct InMemoryHost {
        dump: String,
        configurations: RefCell<HashMap<String, String>>,
        running: RefCell<HashSet<String>>,
        applied_peers: RefCell<Vec<(String, String)>>,
        fail_sync: Cell<bool>,
    }

    impl WireGuardHost for InMemoryHost {
        fn show_all_dump(&self) -> Result<String, String> {
            Ok(self.dump.clone())
        }

        fn list_config_names(&self) -> Result<Vec<String>, String> {
            Ok(self.configurations.borrow().keys().cloned().collect())
        }

        fn read_config(&self, name: &str) -> Result<String, String> {
            self.configurations
                .borrow()
                .get(name)
                .cloned()
                .ok_or_else(|| format!("配置 {name} 不存在"))
        }

        fn write_config(&self, name: &str, content: &str) -> Result<(), String> {
            self.configurations
                .borrow_mut()
                .insert(name.to_string(), content.to_string());
            Ok(())
        }

        fn export_config(
            &self,
            _name: &str,
            _destination: &Path,
            _home: &Path,
        ) -> Result<(), String> {
            Ok(())
        }

        fn interface_details(&self, _name: &str) -> InterfaceDetails {
            InterfaceDetails::default()
        }

        fn interface_stats(&self, _name: &str) -> (u64, u64) {
            (0, 0)
        }

        fn interface_exists(&self, name: &str) -> bool {
            self.running.borrow().contains(name)
        }

        fn apply_interface(&self, name: &str, action: InterfaceAction) -> Result<(), String> {
            match action {
                InterfaceAction::Up | InterfaceAction::Restart => {
                    self.running.borrow_mut().insert(name.to_string());
                }
                InterfaceAction::Down => {
                    self.running.borrow_mut().remove(name);
                }
                InterfaceAction::Sync if self.fail_sync.get() => {
                    return Err("injected sync failure".to_string());
                }
                InterfaceAction::Sync => {}
            }
            Ok(())
        }

        fn set_peers(&self, name: &str, configuration: &str) -> Result<(), String> {
            self.applied_peers
                .borrow_mut()
                .push((name.to_string(), configuration.to_string()));
            Ok(())
        }

        fn generate_private_key(&self) -> Result<String, String> {
            Ok("private".to_string())
        }

        fn derive_public_key(&self, _private_key: &str) -> Result<String, String> {
            Ok("public".to_string())
        }

        fn generate_preshared_key(&self) -> Result<String, String> {
            Ok("preshared".to_string())
        }
    }

    #[test]
    fn apply_config_reports_persistence_when_sync_fails() {
        let host = InMemoryHost::default();
        host.running.borrow_mut().insert("wg0".to_string());
        host.fail_sync.set(true);
        let wireguard = WireGuard::new(host);

        let outcome = wireguard
            .apply_config("wg0.conf", "[Interface]\n", true)
            .unwrap();

        assert!(outcome.persisted);
        assert!(!outcome.runtime_applied);
        assert_eq!(outcome.warnings.len(), 1);
        assert!(wireguard
            .host
            .configurations
            .borrow()
            .contains_key("wg0.conf"));
    }

    #[test]
    fn persistent_peer_apply_preserves_existing_extras() {
        let host = InMemoryHost::default();
        host.running.borrow_mut().insert("wg0".to_string());
        host.configurations.borrow_mut().insert(
            "wg0.conf".to_string(),
            "[Interface]\nAddress = 10.0.0.1/24\n\n[Peer]\nPublicKey = peer-key\nAllowedIPs = 10.0.0.2/32\nCustomPeerKey = keep-me\n".to_string(),
        );
        let wireguard = WireGuard::new(host);
        let peers = vec![PeerConf {
            public_key: "peer-key".to_string(),
            allowed_ips: vec!["10.0.0.3/32".to_string()],
            ..Default::default()
        }];

        let outcome = wireguard
            .apply_peers("wg0", &peers, ApplyMode::PersistAndSync)
            .unwrap();

        assert_eq!(outcome, ApplyOutcome::persisted_and_runtime());
        let saved = wireguard.host.configurations.borrow()["wg0.conf"].clone();
        let parsed = parse_conf(&saved).unwrap();
        assert_eq!(parsed.interface.address, ["10.0.0.1/24"]);
        assert_eq!(
            parsed.peers[0].extras,
            [("CustomPeerKey".to_string(), "keep-me".to_string())]
        );
    }

    #[test]
    fn missing_configuration_falls_back_to_runtime_apply() {
        let wireguard = WireGuard::new(InMemoryHost::default());
        let peers = vec![PeerConf {
            public_key: "peer-key".to_string(),
            allowed_ips: vec!["10.0.0.2/32".to_string()],
            ..Default::default()
        }];

        let outcome = wireguard
            .apply_peers("wg0", &peers, ApplyMode::PersistAndSync)
            .unwrap();

        assert!(!outcome.persisted);
        assert!(outcome.runtime_applied);
        assert_eq!(outcome.warnings.len(), 1);
        assert_eq!(wireguard.host.applied_peers.borrow().len(), 1);
    }

    #[test]
    fn malformed_configuration_is_not_persisted() {
        let wireguard = WireGuard::new(InMemoryHost::default());

        let result = wireguard.apply_config("wg0.conf", "[Unknown]\nValue = broken\n", false);

        assert!(result.is_err());
        assert!(wireguard.host.configurations.borrow().is_empty());
    }

    #[test]
    fn export_path_stays_inside_home() {
        assert_eq!(
            validate_export_path("/home/alice/Downloads/wg0.conf", "/home/alice").unwrap(),
            PathBuf::from("/home/alice/Downloads/wg0.conf")
        );
        assert!(validate_export_path("/etc/wg0.conf", "/home/alice").is_err());
        assert!(validate_export_path("/home/alice/../bob/wg0.conf", "/home/alice").is_err());
    }
}
