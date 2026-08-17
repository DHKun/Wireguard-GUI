import { useEffect, useState } from "react";
import { api, errText } from "../api";
import type { PeerConf } from "../types";

export interface PeerFormState {
  public_key: string;
  preshared_key?: string;
  allowed_ips: string[];
  endpoint?: string;
  persistent_keepalive?: number;
  extras: [string, string][];
}

export function toForm(p: PeerConf): PeerFormState {
  return {
    public_key: p.public_key,
    preshared_key: p.preshared_key || undefined,
    allowed_ips: [...p.allowed_ips],
    endpoint: p.endpoint || undefined,
    persistent_keepalive: p.persistent_keepalive || undefined,
    extras: [...p.extras],
  };
}

export function fromForm(f: PeerFormState): PeerConf {
  return {
    public_key: f.public_key.trim(),
    preshared_key: f.preshared_key?.trim() || undefined,
    allowed_ips: f.allowed_ips.map((s) => s.trim()).filter(Boolean),
    endpoint: f.endpoint?.trim() || undefined,
    persistent_keepalive: f.persistent_keepalive || undefined,
    extras: f.extras,
  };
}

interface Props {
  title: string;
  initial: PeerFormState;
  onSave: (peer: PeerConf) => void;
  onCancel: () => void;
  notify: (msg: string, tone: "ok" | "err") => void;
}

export default function PeerForm({ title, initial, onSave, onCancel, notify }: Props) {
  const [pub, setPub] = useState(initial.public_key);
  const [psk, setPsk] = useState(initial.preshared_key ?? "");
  const [allowed, setAllowed] = useState(initial.allowed_ips.join("\n"));
  const [endpoint, setEndpoint] = useState(initial.endpoint ?? "");
  const [keepalive, setKeepalive] = useState(
    initial.persistent_keepalive ? String(initial.persistent_keepalive) : "",
  );
  const [genPriv, setGenPriv] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setPub(initial.public_key);
    setPsk(initial.preshared_key ?? "");
    setAllowed(initial.allowed_ips.join("\n"));
    setEndpoint(initial.endpoint ?? "");
    setKeepalive(
      initial.persistent_keepalive ? String(initial.persistent_keepalive) : "",
    );
    setGenPriv(null);
  }, [initial]);

  async function genKeypair() {
    setBusy(true);
    setError(null);
    try {
      const kp = await api.generateKeypair();
      setPub(kp.public_key);
      setGenPriv(kp.private_key);
      notify("已生成密钥对，私钥仅显示这一次", "ok");
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function genPsk() {
    setBusy(true);
    setError(null);
    try {
      setPsk(await api.generatePresharedKey());
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  function submit() {
    const trimmed = pub.trim();
    if (!trimmed) return setError("PublicKey 不能为空");
    if (trimmed.length !== 44) return setError("PublicKey 应为 44 字符的 base64 密钥");
    const ips = allowed
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (ips.length === 0) return setError("AllowedIPs 至少填一项");
    let ka: number | undefined;
    if (keepalive.trim() !== "") {
      ka = Number(keepalive);
      if (!Number.isInteger(ka) || ka < 1 || ka > 65535)
        return setError("PersistentKeepalive 需为 1–65535 的整数，留空表示关闭");
    }
    onSave(
      fromForm({
        public_key: trimmed,
        preshared_key: psk.trim() || undefined,
        allowed_ips: ips,
        endpoint: endpoint.trim() || undefined,
        persistent_keepalive: ka,
        extras: initial.extras,
      }),
    );
  }

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="modal">
        <h3>{title}</h3>

        <label className="field">
          <span>PublicKey（必填）</span>
          <div className="row">
            <input
              value={pub}
              onChange={(e) => setPub(e.target.value)}
              placeholder="44 字符 base64 公钥"
              spellCheck={false}
            />
            <button className="btn ghost" onClick={genKeypair} disabled={busy}>
              生成密钥对
            </button>
          </div>
          {genPriv && (
            <div className="gen-priv">
              <span>新私钥（仅显示一次，请立即保存，不会写入任何配置）：</span>
              <code>{genPriv}</code>
            </div>
          )}
        </label>

        <label className="field">
          <span>PresharedKey（可选）</span>
          <div className="row">
            <input
              value={psk}
              onChange={(e) => setPsk(e.target.value)}
              placeholder="44 字符 base64，留空表示无预共享密钥"
              spellCheck={false}
            />
            <button className="btn ghost" onClick={genPsk} disabled={busy}>
              生成
            </button>
          </div>
        </label>

        <label className="field">
          <span>AllowedIPs（必填，每行一个或逗号分隔）</span>
          <textarea
            value={allowed}
            onChange={(e) => setAllowed(e.target.value)}
            rows={3}
            placeholder={"10.66.66.0/24\n10.66.66.1/32"}
            spellCheck={false}
          />
        </label>

        <label className="field">
          <span>Endpoint（可选，host:port）</span>
          <input
            value={endpoint}
            onChange={(e) => setEndpoint(e.target.value)}
            placeholder="vpn.example.com:51820"
            spellCheck={false}
          />
        </label>

        <label className="field">
          <span>PersistentKeepalive 秒（可选，1–65535，留空关闭）</span>
          <input
            value={keepalive}
            onChange={(e) => setKeepalive(e.target.value)}
            placeholder="25"
            inputMode="numeric"
          />
        </label>

        {error && <div className="err-box">{error}</div>}

        <div className="modal-actions">
          <button className="btn" onClick={onCancel}>
            取消
          </button>
          <button className="btn primary" onClick={submit} disabled={busy}>
            确定
          </button>
        </div>
      </div>
    </div>
  );
}
