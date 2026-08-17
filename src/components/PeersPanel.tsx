import { useMemo, useState } from "react";
import { api, errText } from "../api";
import type { InterfaceStatus, PeerConf } from "../types";
import { fmtBytes, handshakeAge, shortKey } from "../utils";
import PeerForm, { toForm, type PeerFormState } from "./PeerForm";

interface Props {
  iface: InterfaceStatus;
  onChanged: () => void;
  notify: (msg: string, tone: "ok" | "err") => void;
}

const emptyPeer = (): PeerFormState => ({
  public_key: "",
  allowed_ips: [],
  extras: [],
});

export default function PeersPanel({ iface, onChanged, notify }: Props) {
  const [draft, setDraft] = useState<PeerFormState[] | null>(null);
  const [editing, setEditing] = useState<number | "new" | null>(null);
  const [syncConf, setSyncConf] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const live = useMemo(
    () =>
      iface.peers.map((p): PeerFormState => ({
        public_key: p.public_key,
        preshared_key: p.preshared_key ?? undefined,
        allowed_ips: [...p.allowed_ips],
        endpoint: p.endpoint ?? undefined,
        persistent_keepalive: p.persistent_keepalive ?? undefined,
        extras: [],
      })),
    [iface.peers],
  );

  const peers = draft ?? live;
  const dirty = draft !== null;

  function startDraft() {
    if (draft === null) setDraft(live.map((p) => ({ ...p, extras: [...p.extras] })));
  }

  function savePeer(peer: PeerConf) {
    const next = draft === null ? live.map((p) => ({ ...p, extras: [...p.extras] })) : draft;
    if (editing === "new") {
      next.push(toForm(peer));
    } else if (typeof editing === "number") {
      next[editing] = toForm(peer);
    }
    setDraft(next);
    setEditing(null);
    setError(null);
  }

  function removePeer(idx: number) {
    startDraft();
    setDraft((d) => (d ? d.filter((_, i) => i !== idx) : d));
  }

  async function apply() {
    if (dirty === false || !draft) return;
    setBusy(true);
    setError(null);
    const target: PeerConf[] = draft.map((f) => ({
      public_key: f.public_key.trim(),
      preshared_key: f.preshared_key?.trim() || undefined,
      allowed_ips: f.allowed_ips.map((s) => s.trim()).filter(Boolean),
      endpoint: f.endpoint?.trim() || undefined,
      persistent_keepalive: f.persistent_keepalive || undefined,
      extras: f.extras,
    }));
    try {
      const confName = `${iface.name}.conf`;
      let hasConf = false;
      try {
        hasConf = (await api.listConfigs()).includes(confName);
      } catch {
        hasConf = false;
      }
      if (syncConf && hasConf) {
        // 读当前配置文件 → 替换 Peer 列表 → 写回 → 热同步到运行中接口
        const conf = await api.readConfigParsed(confName);
        conf.peers = target;
        await api.writeConfigParsed(confName, conf);
        await api.syncconf(iface.name);
        notify(`已写入 ${confName} 并热同步`, "ok");
      } else {
        await api.setPeers(iface.name, target);
        notify(
          syncConf && !hasConf
            ? `未找到 ${confName}，更改仅应用到运行时`
            : `已应用到运行中的 ${iface.name}`,
          "ok",
        );
      }
      setDraft(null);
      setEditing(null);
      onChanged();
    } catch (e) {
      setError(errText(e));
      notify(`操作失败：${errText(e)}`, "err");
    } finally {
      setBusy(false);
    }
  }

  const editingForm =
    editing === "new"
      ? emptyPeer()
      : typeof editing === "number" && peers[editing]
        ? peers[editing]
        : null;

  return (
    <div className="peers-panel">
      {dirty && (
        <div className="sticky-bar">
          <span className="badge-warn">未保存的更改（{peers.length} 个 Peer）</span>
          <label className="chk">
            <input
              type="checkbox"
              checked={syncConf}
              onChange={(e) => setSyncConf(e.target.checked)}
            />
            同时写入 {iface.name}.conf 并热同步
          </label>
          <div className="spacer" />
          <button className="btn" onClick={() => { setDraft(null); setEditing(null); }} disabled={busy}>
            放弃
          </button>
          <button className="btn primary" onClick={apply} disabled={busy}>
            {busy ? "应用中…" : "应用更改"}
          </button>
        </div>
      )}

      <div className="toolbar">
        <button
          className="btn ghost small"
          onClick={() => { startDraft(); setEditing("new"); }}
        >
          ＋ 添加 Peer
        </button>
      </div>

      {error && <div className="err-box">{error}</div>}

      {peers.length === 0 ? (
        <div className="empty">该接口还没有 Peer</div>
      ) : (
        <table className="peer-table">
          <thead>
            <tr>
              <th>公钥</th>
              <th>Endpoint</th>
              <th>AllowedIPs</th>
              <th>最近握手</th>
              <th>流量 ↓ / ↑</th>
              <th>Keepalive</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {peers.map((p, i) => {
              const livePeer = live.find((l) => l.public_key === p.public_key);
              return (
                <tr key={i} className={livePeer ? "" : "row-new"}>
                  <td className="mono">{shortKey(p.public_key)}</td>
                  <td className="mono">{p.endpoint || "—"}</td>
                  <td className="mono small">{p.allowed_ips.join(", ")}</td>
                  <td>
                    {livePeer ? (
                      (() => {
                        const age = handshakeAge(
                          iface.peers.find((x) => x.public_key === p.public_key)
                            ?.latest_handshake ?? 0,
                        );
                        return age ? (
                          <span className="ok">{age}</span>
                        ) : (
                          <span className="muted">从未</span>
                        );
                      })()
                    ) : (
                      <span className="badge-warn">新增</span>
                    )}
                  </td>
                  <td className="mono small">
                    {livePeer
                      ? (() => {
                          const s = iface.peers.find(
                            (x) => x.public_key === p.public_key,
                          );
                          return s
                            ? `${fmtBytes(s.transfer_rx)} / ${fmtBytes(s.transfer_tx)}`
                            : "—";
                        })()
                      : "—"}
                  </td>
                  <td className="mono">{p.persistent_keepalive ? `${p.persistent_keepalive}s` : "—"}</td>
                  <td className="row-actions">
                    <button
                      className="link-btn"
                      onClick={() => { startDraft(); setEditing(i); }}
                    >
                      编辑
                    </button>
                    <button
                      className="link-btn danger"
                      onClick={() => removePeer(i)}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}

      {editingForm && (
        <PeerForm
          title={
            editing === "new"
              ? `添加 Peer 到 ${iface.name}`
              : `编辑 Peer（${iface.name}）`
          }
          initial={editingForm}
          onSave={savePeer}
          onCancel={() => setEditing(null)}
          notify={notify}
        />
      )}
    </div>
  );
}
