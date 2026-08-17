import type { ApplyMode, ApplyOutcome, PeerConf, PeerStatus } from "../types";
import type { WireGuardPort } from "../wireguard";

export type PeerEditTarget = number | "new";

function clonePeer(peer: PeerConf): PeerConf {
  return {
    ...peer,
    allowed_ips: [...peer.allowed_ips],
    extras: peer.extras.map(([key, value]) => [key, value]),
  };
}

function normalize(peer: PeerConf): PeerConf {
  return {
    public_key: peer.public_key.trim(),
    preshared_key: peer.preshared_key?.trim() || undefined,
    allowed_ips: peer.allowed_ips.map((value) => value.trim()).filter(Boolean),
    endpoint: peer.endpoint?.trim() || undefined,
    persistent_keepalive: peer.persistent_keepalive || undefined,
    extras: peer.extras.map(([key, value]) => [key, value]),
  };
}

/** Owns Peer drafts, validation, normalization, and application. */
export class PeerEditing {
  constructor(private readonly port: WireGuardPort) {}

  empty(): PeerConf {
    return { public_key: "", allowed_ips: [], extras: [] };
  }

  draft(statuses: PeerStatus[]): PeerConf[] {
    return statuses.map((peer) => ({
      public_key: peer.public_key,
      preshared_key: peer.preshared_key ?? undefined,
      allowed_ips: [...peer.allowed_ips],
      endpoint: peer.endpoint ?? undefined,
      persistent_keepalive: peer.persistent_keepalive ?? undefined,
      extras: [],
    }));
  }

  save(draft: PeerConf[], target: PeerEditTarget, peer: PeerConf): PeerConf[] {
    const next = draft.map(clonePeer);
    const normalized = normalize(peer);
    const validation = this.validate(normalized);
    if (validation) throw new Error(validation);
    if (target === "new") next.push(normalized);
    else next[target] = normalized;
    return next;
  }

  remove(draft: PeerConf[], index: number): PeerConf[] {
    return draft.filter((_, current) => current !== index).map(clonePeer);
  }

  validate(peer: PeerConf): string | null {
    if (!peer.public_key.trim()) return "PublicKey 不能为空";
    if (peer.public_key.trim().length !== 44) return "PublicKey 应为 44 字符的 base64 密钥";
    if (peer.allowed_ips.map((value) => value.trim()).filter(Boolean).length === 0) {
      return "AllowedIPs 至少填一项";
    }
    const keepalive = peer.persistent_keepalive;
    if (
      keepalive !== undefined &&
      (!Number.isInteger(keepalive) || keepalive < 1 || keepalive > 65535)
    ) {
      return "PersistentKeepalive 需为 1–65535 的整数，留空表示关闭";
    }
    return null;
  }

  async apply(name: string, draft: PeerConf[], mode: ApplyMode): Promise<ApplyOutcome> {
    const peers = draft.map(normalize);
    const keys = new Set<string>();
    for (const peer of peers) {
      const validation = this.validate(peer);
      if (validation) throw new Error(validation);
      if (keys.has(peer.public_key)) throw new Error(`Peer 公钥重复: ${peer.public_key}`);
      keys.add(peer.public_key);
    }
    return this.port.applyPeers(name, peers, mode);
  }

  generateKeypair() {
    return this.port.generateKeypair();
  }

  generatePresharedKey() {
    return this.port.generatePresharedKey();
  }
}
