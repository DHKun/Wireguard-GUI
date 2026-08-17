import { useState } from "react";
import type { InterfaceAction, InterfaceStatus, ToastTone } from "../types";
import { fmtBytes, fmtRate, maskKey } from "../utils";
import { errText, wireguard } from "../wireguard";
import PeersPanel from "./PeersPanel";

interface Props {
  iface: InterfaceStatus;
  rates: { rx: number; tx: number } | null;
  onChanged: () => void;
  notify: (msg: string, tone: ToastTone) => void;
}

export default function InterfaceCard({ iface, rates, onChanged, notify }: Props) {
  const [showPeers, setShowPeers] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [revealKey, setRevealKey] = useState(false);

  async function run(action: InterfaceAction) {
    setBusy(action);
    const label =
      action === "up" ? "启动" :
      action === "down" ? "停止" :
      action === "restart" ? "重启" : "热同步";
    try {
      await wireguard.applyInterface(iface.name, action);
      notify(`${iface.name} ${label}成功`, "ok");
      onChanged();
    } catch (e) {
      notify(`${label}失败：${errText(e)}`, "err");
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="card">
      <div className="card-head">
        <div className="iface-title">
          <span className="iface-name mono">{iface.name}</span>
          <span className={`dot ${iface.running ? "on" : ""}`} />
          <span className="muted">{iface.addresses.join(", ") || "无地址"}</span>
        </div>
        <div className="card-actions">
          {iface.running ? (
            <>
              <button className="btn ghost small" onClick={() => run("sync")} disabled={busy !== null}>
                {busy === "sync" ? "…" : "热同步"}
              </button>
              <button className="btn ghost small" onClick={() => run("restart")} disabled={busy !== null}>
                {busy === "restart" ? "…" : "重启"}
              </button>
              <button className="btn ghost small danger" onClick={() => run("down")} disabled={busy !== null}>
                {busy === "down" ? "…" : "停止"}
              </button>
            </>
          ) : (
            <button className="btn primary small" onClick={() => run("up")} disabled={busy !== null}>
              {busy === "up" ? "…" : "启动"}
            </button>
          )}
        </div>
      </div>

      <div className="iface-meta">
        <div className="kv">
          <span className="k">公钥</span>
          <span className="v mono">{maskKey(iface.public_key, revealKey)}</span>
          <button className="link-btn" onClick={() => setRevealKey(!revealKey)}>
            {revealKey ? "隐藏" : "显示"}
          </button>
        </div>
        <div className="kv"><span className="k">监听端口</span><span className="v mono">{iface.listen_port || "—"}</span></div>
        <div className="kv"><span className="k">MTU</span><span className="v mono">{iface.mtu ?? "—"}</span></div>
        <div className="kv"><span className="k">FwMark</span><span className="v mono">{iface.fwmark ?? "—"}</span></div>
        <div className="kv">
          <span className="k">流量</span>
          <span className="v mono">
            ↓ {fmtBytes(iface.rx_bytes)} {rates && `(${fmtRate(rates.rx)})`}
            {"  "}↑ {fmtBytes(iface.tx_bytes)} {rates && `(${fmtRate(rates.tx)})`}
          </span>
        </div>
        <div className="kv"><span className="k">Peer 数</span><span className="v">{iface.peers.length}</span></div>
      </div>

      <div className="card-subhead">
        <button className="link-btn" onClick={() => setShowPeers(!showPeers)}>
          {showPeers ? "▾" : "▸"} Peer 列表（{iface.peers.length}）
        </button>
      </div>

      {showPeers && (
        <PeersPanel iface={iface} onChanged={onChanged} notify={notify} />
      )}
    </div>
  );
}
