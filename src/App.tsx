import { useCallback, useEffect, useMemo, useState } from "react";
import ConfigPanel from "./components/ConfigPanel";
import InterfaceCard from "./components/InterfaceCard";
import SettingsPanel from "./components/SettingsPanel";
import { StatusMonitor, type MonitorSnapshot } from "./domain/statusMonitor";
import type { ToastTone } from "./types";
import { wireguard } from "./wireguard";
import "./App.css";

interface Toast {
  id: number;
  msg: string;
  tone: ToastTone;
}

const REFRESH_MS = 5000;

export default function App() {
  const monitor = useMemo(() => new StatusMonitor(wireguard), []);
  const [snapshot, setSnapshot] = useState<MonitorSnapshot>({
    interfaces: [],
    rates: {},
    loading: true,
    stale: false,
    error: null,
    lastUpdated: null,
  });
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [tab, setTab] = useState<"dash" | "config" | "settings">("dash");
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [envOk, setEnvOk] = useState<boolean | null>(null);
  const [envDetail, setEnvDetail] = useState<string>("");

  const notify = useCallback((msg: string, tone: ToastTone = "ok") => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-3), { id, msg, tone }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 5000);
  }, []);

  const refresh = useCallback(() => void monitor.refresh(true), [monitor]);

  useEffect(() => monitor.subscribe(setSnapshot), [monitor]);

  useEffect(() => {
    void monitor.checkEnvironment().then((status) => {
      setEnvOk(status.ok);
      setEnvDetail(status.detail);
    });
  }, [monitor]);

  useEffect(() => {
    return monitor.start(autoRefresh ? REFRESH_MS : null);
  }, [monitor, autoRefresh]);

  const { interfaces, rates, loading, error, lastUpdated } = snapshot;

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <img className="logo" src="/app-icon.png" alt="" aria-hidden="true" />
          <h1>WireGuard 控制台</h1>
          {envOk === false && (
            <span className="badge-warn" title={envDetail}>
              ⚠ 环境异常
            </span>
          )}
        </div>
        <nav className="tabs">
          <button className={tab === "dash" ? "tab active" : "tab"} onClick={() => setTab("dash")}>
            仪表盘
          </button>
          <button className={tab === "config" ? "tab active" : "tab"} onClick={() => setTab("config")}>
            配置
          </button>
          <button className={tab === "settings" ? "tab active" : "tab"} onClick={() => setTab("settings")}>
            设置
          </button>
        </nav>
        <div className="topbar-right">
          {lastUpdated !== null && (
            <span className="muted small">{new Date(lastUpdated).toLocaleTimeString()} 更新</span>
          )}
          <label className="chk">
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
            />
            自动刷新
          </label>
          <button
            className="btn ghost small"
            onClick={refresh}
            disabled={loading}
          >
            {loading ? "…" : "刷新"}
          </button>
        </div>
      </header>

      {error && (
        <div className="err-banner">
          <b>操作失败：</b>
          {error}
          <button className="link-btn" onClick={refresh}>
            重试
          </button>
        </div>
      )}

      <main>
        {tab === "dash" ? (
          <div className="dash">
            {loading && interfaces.length === 0 ? (
              <div className="empty tall">正在读取接口状态…</div>
            ) : interfaces.length === 0 ? (
              <div className="empty tall">
                没有运行中的 WireGuard 接口
                <div className="hint">
                  使用「配置」页写入 /etc/wireguard/*.conf 后即可在仪表盘启动
                </div>
              </div>
            ) : (
              interfaces.map((iface) => (
                <InterfaceCard
                  key={iface.name}
                  iface={iface}
                  rates={rates[iface.name] ?? null}
                  onChanged={refresh}
                  notify={notify}
                />
              ))
            )}
          </div>
        ) : tab === "config" ? (
          <ConfigPanel notify={notify} />
        ) : (
          <SettingsPanel notify={notify} />
        )}
      </main>

      <div className="toasts">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.tone}`}>
            {t.msg}
          </div>
        ))}
      </div>
    </div>
  );
}
