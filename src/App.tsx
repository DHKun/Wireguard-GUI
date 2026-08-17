import { useCallback, useEffect, useRef, useState } from "react";
import { api, errText } from "./api";
import ConfigPanel from "./components/ConfigPanel";
import InterfaceCard from "./components/InterfaceCard";
import type { InterfaceStatus } from "./types";
import "./App.css";

interface Toast {
  id: number;
  msg: string;
  tone: "ok" | "err";
}

interface Rates {
  rx: number;
  tx: number;
}

const REFRESH_MS = 5000;

export default function App() {
  const [interfaces, setInterfaces] = useState<InterfaceStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const [tab, setTab] = useState<"dash" | "config">("dash");
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [envOk, setEnvOk] = useState<boolean | null>(null);
  const [envDetail, setEnvDetail] = useState<string>("");

  const prevRef = useRef<Map<string, { time: number; rx: number; tx: number }>>(
    new Map(),
  );
  const ratesRef = useRef<Map<string, Rates>>(new Map());
  // 一次只允许一个刷新在途（pkexec 授权框挂起时不再堆叠新请求）
  const inFlightRef = useRef(false);
  // 授权失败/取消后的冷却期，避免每 5s 弹一次 polkit 对话框
  const cooldownRef = useRef(0);

  const notify = useCallback((msg: string, tone: "ok" | "err" = "ok") => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-3), { id, msg, tone }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 5000);
  }, []);

  const refresh = useCallback(async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    try {
      const list = await api.wgStatus();
      const now = Date.now();
      const prev = prevRef.current;
      const rates = ratesRef.current;
      for (const iface of list) {
        const old = prev.get(iface.name);
        if (old && now > old.time) {
          const dt = (now - old.time) / 1000;
          rates.set(iface.name, {
            rx: Math.max(0, (iface.rx_bytes - old.rx) / dt),
            tx: Math.max(0, (iface.tx_bytes - old.tx) / dt),
          });
        } else {
          rates.set(iface.name, { rx: 0, tx: 0 });
        }
        prev.set(iface.name, { time: now, rx: iface.rx_bytes, tx: iface.tx_bytes });
      }
      setInterfaces(list);
      setLastUpdated(new Date());
      setError(null);
    } catch (e) {
      setError(errText(e));
      // 授权被取消或失败：60 秒内不再自动触发（手动刷新仍可用）
      cooldownRef.current = Date.now() + 60_000;
    } finally {
      setLoading(false);
      inFlightRef.current = false;
    }
  }, []);

  // 环境检查
  useEffect(() => {
    api
      .checkEnv()
      .then((env) => {
        const missing: string[] = [];
        if (!env.wg) missing.push("wg");
        if (!env.wg_quick) missing.push("wg-quick");
        if (!env.pkexec) missing.push("pkexec");
        if (!env.conf_dir_exists) missing.push("/etc/wireguard");
        if (missing.length) {
          setEnvOk(false);
          setEnvDetail(`缺少：${missing.join(", ")}`);
        } else {
          setEnvOk(true);
        }
      })
      .catch(() => {
        setEnvOk(false);
        setEnvDetail("环境检查失败");
      });
  }, []);

  // 首次加载 + 自动刷新（冷却期内跳过；在途请求不叠加）
  useEffect(() => {
    refresh();
    if (!autoRefresh) return;
    const timer = setInterval(() => {
      if (Date.now() < cooldownRef.current) return;
      refresh();
    }, REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh, autoRefresh]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="logo">⬡</span>
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
        </nav>
        <div className="topbar-right">
          {lastUpdated && (
            <span className="muted small">{lastUpdated.toLocaleTimeString()} 更新</span>
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
                  rates={ratesRef.current.get(iface.name) ?? null}
                  onChanged={refresh}
                  notify={notify}
                />
              ))
            )}
          </div>
        ) : (
          <ConfigPanel notify={notify} />
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
