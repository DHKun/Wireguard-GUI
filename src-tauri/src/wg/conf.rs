//! wg-quick 配置文件（/etc/wireguard/*.conf）解析与序列化。
//!
//! 支持注释（# 与 ;）、多值键（Address/DNS/AllowedIPs/PostUp 等）、
//! 未知键原样保留（extras），保证读写不丢数据。序列化时注释会丢失。

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InterfaceConf {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_up: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_up: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_down: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_down: Vec<String>,
    /// 未知键（如 FwMark、SaveConfig、RoutingTable 等），保序保留
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PeerConf {
    pub public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preshared_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_ips: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistent_keepalive: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WgConf {
    pub interface: InterfaceConf,
    #[serde(default)]
    pub peers: Vec<PeerConf>,
}

/// 去掉行内注释（# 或 ; 开头的部分）。注意值中不允许出现这两个字符。
fn strip_comment(line: &str) -> &str {
    match line.find(['#', ';']) {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn parse_kv(line: &str) -> Option<(String, String)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    let value = line[eq + 1..].trim();
    if key.is_empty() {
        return None;
    }
    Some((key.to_string(), value.to_string()))
}

/// 解析 wg-quick 配置文本。
pub fn parse_conf(text: &str) -> Result<WgConf, String> {
    let mut conf = WgConf::default();
    let mut section: u8 = 0; // 0=none, 1=interface, 2=peer
    let mut cur_peer: Option<usize> = None;

    for (lineno, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let lnum = lineno + 1;

        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(format!("第 {lnum} 行段头格式错误: {line}"));
            }
            let name = line[1..line.len() - 1].trim();
            match name {
                "Interface" => section = 1,
                "Peer" => {
                    section = 2;
                    conf.peers.push(PeerConf::default());
                    cur_peer = Some(conf.peers.len() - 1);
                }
                other => return Err(format!("第 {lnum} 行未知段 [{other}]")),
            }
            continue;
        }

        let (key, value) = parse_kv(line)
            .ok_or_else(|| format!("第 {lnum} 行不是 key = value 格式: {line}"))?;
        if value.is_empty() {
            continue;
        }

        match section {
            1 => apply_interface_key(&mut conf.interface, &key, &value),
            2 => {
                let idx = cur_peer.ok_or_else(|| format!("第 {lnum} 行 Peer 键出现在 Peer 段之外"))?;
                apply_peer_key(&mut conf.peers[idx], &key, &value);
            }
            _ => return Err(format!("第 {lnum} 行键出现在任何段之外: {line}")),
        }
    }

    Ok(conf)
}

/// 逗号分隔的多值键（Address/DNS/AllowedIPs 与 wg-quick 语义一致）。
fn split_multi(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn apply_interface_key(iface: &mut InterfaceConf, key: &str, value: &str) {
    match key {
        "Address" => iface.address.extend(split_multi(value)),
        "ListenPort" => iface.listen_port = value.parse().ok(),
        "PrivateKey" => iface.private_key = Some(value.to_string()),
        "DNS" => iface.dns.extend(split_multi(value)),
        "MTU" => iface.mtu = value.parse().ok(),
        "Table" => iface.table = Some(value.to_string()),
        "PreUp" => iface.pre_up.push(value.to_string()),
        "PostUp" => iface.post_up.push(value.to_string()),
        "PreDown" => iface.pre_down.push(value.to_string()),
        "PostDown" => iface.post_down.push(value.to_string()),
        other => iface.extras.push((other.to_string(), value.to_string())),
    }
}

fn apply_peer_key(peer: &mut PeerConf, key: &str, value: &str) {
    match key {
        "PublicKey" => peer.public_key = value.to_string(),
        "PresharedKey" => peer.preshared_key = Some(value.to_string()),
        "AllowedIPs" => peer.allowed_ips.extend(split_multi(value)),
        "Endpoint" => peer.endpoint = Some(value.to_string()),
        "PersistentKeepalive" => peer.persistent_keepalive = value.parse().ok(),
        other => peer.extras.push((other.to_string(), value.to_string())),
    }
}

fn emit_kv(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(value);
    out.push('\n');
}

fn emit_multi(out: &mut String, key: &str, values: &[String]) {
    for v in values {
        emit_kv(out, key, v);
    }
}

/// 序列化为 wg-quick 兼容文本（canonical 顺序，注释不保留）。
pub fn serialize_conf(conf: &WgConf) -> String {
    let mut out = String::new();
    let i = &conf.interface;

    out.push_str("[Interface]\n");
    emit_multi(&mut out, "Address", &i.address);
    if let Some(p) = i.listen_port {
        emit_kv(&mut out, "ListenPort", &p.to_string());
    }
    if let Some(k) = &i.private_key {
        emit_kv(&mut out, "PrivateKey", k);
    }
    emit_multi(&mut out, "DNS", &i.dns);
    if let Some(m) = i.mtu {
        emit_kv(&mut out, "MTU", &m.to_string());
    }
    if let Some(t) = &i.table {
        emit_kv(&mut out, "Table", t);
    }
    emit_multi(&mut out, "PreUp", &i.pre_up);
    emit_multi(&mut out, "PostUp", &i.post_up);
    emit_multi(&mut out, "PreDown", &i.pre_down);
    emit_multi(&mut out, "PostDown", &i.post_down);
    for (k, v) in &i.extras {
        emit_kv(&mut out, k, v);
    }
    if conf.peers.is_empty() {
        return out;
    }
    out.push('\n');

    for (n, p) in conf.peers.iter().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        out.push_str("[Peer]\n");
        emit_kv(&mut out, "PublicKey", &p.public_key);
        if let Some(k) = &p.preshared_key {
            emit_kv(&mut out, "PresharedKey", k);
        }
        emit_multi(&mut out, "AllowedIPs", &p.allowed_ips);
        if let Some(e) = &p.endpoint {
            emit_kv(&mut out, "Endpoint", e);
        }
        if let Some(k) = p.persistent_keepalive {
            emit_kv(&mut out, "PersistentKeepalive", &k.to_string());
        }
        for (k, v) in &p.extras {
            emit_kv(&mut out, k, v);
        }
    }

    out
}

/// 仅由 Peer 段组成、供 `wg setconf` 使用的文本。
pub fn serialize_peers_for_setconf(peers: &[PeerConf]) -> String {
    let mut out = String::new();
    for p in peers {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[Peer]\n");
        emit_kv(&mut out, "PublicKey", &p.public_key);
        if let Some(k) = &p.preshared_key {
            emit_kv(&mut out, "PresharedKey", k);
        }
        emit_multi(&mut out, "AllowedIPs", &p.allowed_ips);
        if let Some(e) = &p.endpoint {
            emit_kv(&mut out, "Endpoint", e);
        }
        if let Some(k) = p.persistent_keepalive {
            emit_kv(&mut out, "PersistentKeepalive", &k.to_string());
        }
        for (k, v) in &p.extras {
            emit_kv(&mut out, k, v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_full_conf() {
        let src = "\
# 我的 VPN 服务端
[Interface]
Address = 10.66.66.5/32
ListenPort = 51820
PrivateKey = PRIV123
MTU = 1420
PostUp = iptables -A FORWARD -i wg0 -j ACCEPT
FwMark = 0xca6c

[Peer]
PublicKey = PEER000
PresharedKey = PSK000
AllowedIPs = 10.66.66.0/24, 10.66.66.1/32
Endpoint = vpn.example.com:51820
PersistentKeepalive = 25

[Peer]
PublicKey = PEER001
AllowedIPs = 10.66.67.0/24
";
        let conf = parse_conf(src).unwrap();
        assert_eq!(conf.interface.address, vec!["10.66.66.5/32"]);
        assert_eq!(conf.interface.listen_port, Some(51820));
        assert_eq!(conf.interface.mtu, Some(1420));
        assert_eq!(conf.interface.post_up.len(), 1);
        assert_eq!(conf.interface.extras, vec![("FwMark".into(), "0xca6c".into())]);
        assert_eq!(conf.peers.len(), 2);
        assert_eq!(conf.peers[0].allowed_ips.len(), 2);
        assert_eq!(conf.peers[0].endpoint.as_deref(), Some("vpn.example.com:51820"));
        assert!(conf.peers[1].preshared_key.is_none());

        let out = serialize_conf(&conf);
        let reparsed = parse_conf(&out).unwrap();
        assert_eq!(reparsed.interface.address, conf.interface.address);
        assert_eq!(reparsed.interface.extras, conf.interface.extras);
        assert_eq!(reparsed.peers.len(), 2);
        assert_eq!(reparsed.peers[0].allowed_ips, conf.peers[0].allowed_ips);
        assert_eq!(reparsed.peers[1].allowed_ips, conf.peers[1].allowed_ips);
    }

    #[test]
    fn setconf_text_has_no_interface() {
        let peers = vec![PeerConf {
            public_key: "K".into(),
            allowed_ips: vec!["10.0.0.2/32".into()],
            ..Default::default()
        }];
        let text = serialize_peers_for_setconf(&peers);
        assert!(!text.contains("[Interface]"));
        assert!(text.contains("[Peer]"));
    }
}
