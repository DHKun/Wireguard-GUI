import { useMemo, useState } from "react";
import { describeApplyOutcome } from "../domain/configuration";
import { PeerEditing, type PeerEditTarget } from "../domain/peerEditing";
import type { InterfaceStatus, PeerConf, ToastTone } from "../types";
import { fmtBytes, handshakeAge, shortKey } from "../utils";
import { errText, wireguard } from "../wireguard";
import PeerForm from "./PeerForm";

interface Props {
  iface: InterfaceStatus;
  onChanged: () => void;
  notify: (msg: string, tone: ToastTone) => void;
}

const peerEditing = new PeerEditing(wireguard);

export default function PeersPanel({ iface, onChanged, notify }: Props) {
  const [draft, setDraft] = useState<PeerConf[] | null>(null);
  const [editing, setEditing] = useState<PeerEditTarget | null>(null);
  const [syncConf, setSyncConf] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const live = useMemo(() => peerEditing.draft(iface.peers), [iface.peers]);

  const peers = draft ?? live;
  const dirty = draft !== null;

  function startDraft() {
    if (draft === null) setDraft(peerEditing.draft(iface.peers));
  }

  function savePeer(peer: PeerConf) {
    if (editing === null) return;
    try {
      setDraft(peerEditing.save(draft ?? peerEditing.draft(iface.peers), editing, peer));
      setEditing(null);
      setError(null);
    } catch (cause) {
      setError(errText(cause));
    }
  }

  function removePeer(idx: number) {
    setDraft(peerEditing.remove(draft ?? peerEditing.draft(iface.peers), idx));
  }

  async function apply() {
    if (dirty === false || !draft) return;
    setBusy(true);
    setError(null);
    try {
      const outcome = await peerEditing.apply(
        iface.name,
        draft,
        syncConf ? "persist_and_sync" : "runtime_only",
      );
      const success = outcome.persisted
        ? `已持久化 Peer 更改并同步 ${iface.name}`
        : `已应用到运行中的 ${iface.name}`;
      notify(
        describeApplyOutcome(success, outcome),
        outcome.warnings.length ? "warn" : "ok",
      );
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
      ? peerEditing.empty()
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
