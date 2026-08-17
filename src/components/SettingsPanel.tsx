import { useEffect, useState } from "react";
import { AppPreferences } from "../domain/appSettings";
import type { AppSettings, ToastTone } from "../types";
import { errText, wireguard } from "../wireguard";

interface Props {
  notify: (message: string, tone: ToastTone) => void;
}

const preferences = new AppPreferences(wireguard);

export default function SettingsPanel({ notify }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void preferences
      .load()
      .then(setSettings)
      .catch((cause) => setError(errText(cause)));
  }, []);

  async function update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (!settings) return;
    const next = preferences.toggle(settings, key, value);
    setSettings(next);
    setBusy(true);
    setError(null);
    try {
      setSettings(await preferences.save(next));
      notify("设置已保存", "ok");
    } catch (cause) {
      const message = errText(cause);
      setError(message);
      notify(`保存设置失败：${message}`, "err");
      setSettings(await preferences.load().catch(() => settings));
    } finally {
      setBusy(false);
    }
  }

  if (settings === null) {
    return <div className="empty tall">{error ?? "正在读取设置…"}</div>;
  }

  return (
    <div className="settings-panel">
      <section className="settings-card">
        <h2>启动</h2>
        <label className="settings-row">
          <span>
            <strong>开机自启</strong>
            <span className="muted">登录后自动启动本程序</span>
          </span>
          <input
            type="checkbox"
            checked={settings.autostart}
            disabled={busy}
            onChange={(event) => update("autostart", event.target.checked)}
          />
        </label>
        <label className="settings-row">
          <span>
            <strong>静默启动</strong>
            <span className="muted">开机自启时不弹出窗口，只出现在托盘</span>
          </span>
          <input
            type="checkbox"
            checked={settings.silent_start}
            disabled={busy || !settings.autostart}
            onChange={(event) => update("silent_start", event.target.checked)}
          />
        </label>
      </section>

      <section className="settings-card">
        <h2>关闭窗口</h2>
        <label className="settings-choice">
          <input
            type="radio"
            name="close-behavior"
            checked={!settings.close_to_tray}
            disabled={busy}
            onChange={() => update("close_to_tray", false)}
          />
          <span>
            <strong>直接退出</strong>
            <span className="muted">关闭窗口即结束进程</span>
          </span>
        </label>
        <label className="settings-choice">
          <input
            type="radio"
            name="close-behavior"
            checked={settings.close_to_tray}
            disabled={busy}
            onChange={() => update("close_to_tray", true)}
          />
          <span>
            <strong>最小化到后台</strong>
            <span className="muted">关闭窗口后继续在托盘运行，左键托盘可恢复</span>
          </span>
        </label>
      </section>

      {error && <div className="err-box">{error}</div>}
    </div>
  );
}
