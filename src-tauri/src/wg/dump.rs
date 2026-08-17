//! `wg show all dump`（机器可读，tab 分隔）解析。
//!
//! 格式（man wg）：
//!   interface 行: iface \t private-key \t public-key \t listen_port \t fwmark
//!   peer 行:     iface \t public-key \t preshared \t endpoint \t allowed_ips \t latest_handshake \t transfer_rx \t transfer_tx \t persistent_keepalive
//!
//! 非 root 运行时 privkey/preshared 显示为 "(hidden)"；我们经 pkexec 以 root 运行，可拿到真实值。

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct PeerStatus {
    pub public_key: String,
    pub preshared_key: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    /// 最近握手时间（Unix 秒），0 表示从未握手
    pub latest_handshake: u64,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
    pub persistent_keepalive: Option<u16>,
}

#[derive(Serialize, Clone, Debug)]
pub struct InterfaceDump {
    pub name: String,
    pub public_key: String,
    pub private_key: Option<String>,
    pub listen_port: u16,
    pub fwmark: Option<String>,
    pub peers: Vec<PeerStatus>,
}

fn parse_u64(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

fn parse_opt_u16(s: &str) -> Option<u16> {
    match s.trim() {
        "off" | "" | "(hidden)" => None,
        v => v.parse().ok(),
    }
}

/// 解析 dump 文本为接口列表（按出现顺序）。
pub fn parse_dump(text: &str) -> Result<Vec<InterfaceDump>, String> {
    let mut interfaces: Vec<InterfaceDump> = Vec::new();
    let mut current: Option<usize> = None;

    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            return Err(format!("dump 第 {} 行字段不足: {line:?}", lineno + 1));
        }

        // 接口行固定 5 字段、Peer 行固定 9 字段（dump 格式约定，空字段也占位）。
        // 接口行顺序（man wg）：name \t private-key \t public-key \t listen-port \t fwmark
        if parts.len() == 5 {
            let name = parts[0].to_string();
            let idx = interfaces.len();
            interfaces.push(InterfaceDump {
                name: name.clone(),
                private_key: match parts[1] {
                    "(hidden)" => None,
                    other if !other.is_empty() => Some(other.to_string()),
                    _ => None,
                },
                public_key: parts[2].to_string(),
                listen_port: parts[3].trim().parse().unwrap_or(0),
                fwmark: match parts[4] {
                    "off" => None,
                    other if !other.is_empty() => Some(other.to_string()),
                    _ => None,
                },
                peers: Vec::new(),
            });
            current = Some(idx);
        } else if parts.len() == 9 {
            // peer 行
            let idx =
                current.ok_or_else(|| format!("dump 第 {} 行出现孤立 peer 记录", lineno + 1))?;
            let iface = &mut interfaces[idx];
            iface.peers.push(PeerStatus {
                public_key: parts[1].to_string(),
                preshared_key: match parts[2] {
                    "(hidden)" | "" => None,
                    other => Some(other.to_string()),
                },
                endpoint: match parts[3] {
                    "" | "(no endpoint)" => None,
                    e => Some(e.to_string()),
                },
                allowed_ips: parts[4]
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .collect(),
                latest_handshake: parse_u64(parts[5]),
                transfer_rx: parse_u64(parts[6]),
                transfer_tx: parse_u64(parts[7]),
                persistent_keepalive: parse_opt_u16(parts[8]),
            });
        } else {
            return Err(format!("dump 第 {} 行无法识别: {line:?}", lineno + 1));
        }
    }

    Ok(interfaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dump() {
        // 按 man wg 的 dump 字段顺序构造：接口行 = name, private-key, public-key, port, fwmark
        let privkey = "B".repeat(44);
        let pubkey = "A".repeat(44);
        let peer_a = "C".repeat(44);
        let peer_b = "D".repeat(44);
        let psk = "E".repeat(44);
        let sample = format!(
            "wg0\t{privkey}\t{pubkey}\t51820\toff\n\
             wg0\t{peer_a}\t{psk}\t1.2.3.4:51820\t10.66.66.0/24,10.66.66.1/32\t1700000000\t1024\t2048\toff\n\
             wg0\t{peer_b}\t\t(no endpoint)\t10.0.0.2/32\t0\t0\t0\t25\n"
        );
        let parsed = parse_dump(&sample).unwrap();
        assert_eq!(parsed.len(), 1);
        let iface = &parsed[0];
        assert_eq!(iface.name, "wg0");
        assert_eq!(iface.listen_port, 51820);
        assert!(iface.fwmark.is_none());
        // 字段顺序：私钥在前、公钥在后
        assert_eq!(iface.private_key.as_deref(), Some(privkey.as_str()));
        assert_eq!(iface.public_key, pubkey);
        assert_eq!(iface.peers.len(), 2);
        assert_eq!(iface.peers[0].endpoint.as_deref(), Some("1.2.3.4:51820"));
        assert_eq!(iface.peers[0].allowed_ips.len(), 2);
        assert_eq!(iface.peers[0].transfer_rx, 1024);
        assert!(iface.peers[1].endpoint.is_none());
        assert_eq!(iface.peers[1].persistent_keepalive, Some(25));
        assert_eq!(iface.peers[0].preshared_key.as_deref(), Some(psk.as_str()));
    }
}
