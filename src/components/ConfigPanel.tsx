import { useEffect, useState } from "react";
import {
  ConfigurationLifecycle,
  isConfigurationName,
  type ConfigurationDocument,
} from "../domain/configuration";
import type { ToastTone } from "../types";
import { errText, wireguard } from "../wireguard";

interface Props {
  notify: (message: string, tone: ToastTone) => void;
}

const configuration = new ConfigurationLifecycle(wireguard);

export default function ConfigPanel({ notify }: Props) {
  const [configs, setConfigs] = useState<string[]>([]);
  const [document, setDocument] = useState<ConfigurationDocument | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      const list = await configuration.list();
      setConfigs(list);
      setDocument((current) =>
        current?.source === "stored" && !list.includes(current.name) ? null : current,
      );
    } catch (cause) {
      setError(errText(cause));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function open(name: string) {
    if (
      document?.dirty &&
      !window.confirm(`放弃对 ${document.name || "新配置"} 的未保存修改？`)
    ) {
      return;
    }
    setBusy("load");
    setError(null);
    try {
      setDocument(await configuration.open(name));
    } catch (cause) {
      setError(errText(cause));
    } finally {
      setBusy(null);
    }
  }

  async function save(synchronize: boolean) {
    if (!document) return;
    setBusy(synchronize ? "apply" : "save");
    setError(null);
    try {
      const result = await configuration.apply(document, synchronize);
      setDocument(result.document);
      await refresh();
      notify(result.message, result.outcome.warnings.length ? "warn" : "ok");
    } catch (cause) {
      const message = errText(cause);
      setError(message);
      notify(`保存失败：${message}`, "err");
    } finally {
      setBusy(null);
    }
  }

  function importFile() {
    const input = window.document.createElement("input");
    input.type = "file";
    input.accept = ".conf,text/plain";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        setDocument(configuration.imported(file.name, await file.text()));
        setError(null);
        notify(`已载入 ${file.name}，请确认配置名后保存`, "ok");
      } catch (cause) {
        notify(`读取文件失败：${errText(cause)}`, "err");
      }
    };
    input.click();
  }

  async function exportFile() {
    if (!document || document.source !== "stored") return;
    setBusy("export");
    try {
      const environment = await wireguard.checkEnvironment();
      const destination = window.prompt(
        `导出 ${document.name} 到 HOME 内路径：`,
        `${environment.home}/Downloads/${document.name}`,
      );
      if (!destination) return;
      await configuration.export(document, destination);
      notify(`已导出到 ${destination}`, "ok");
    } catch (cause) {
      notify(`导出失败：${errText(cause)}`, "err");
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="config-panel">
      <div className="config-side">
        <div className="toolbar">
          <button className="btn ghost small" onClick={refresh} disabled={busy !== null}>
            刷新列表
          </button>
          <button className="btn ghost small" onClick={importFile} disabled={busy !== null}>
            导入
          </button>
        </div>
        {configs.length === 0 ? (
          <div className="empty">/etc/wireguard 下暂无 .conf 文件</div>
        ) : (
          <ul className="config-list">
            {configs.map((name) => (
              <li key={name}>
                <button
                  className={`config-item mono ${
                    document?.source === "stored" && document.name === name ? "active" : ""
                  }`}
                  onClick={() => open(name)}
                >
                  {name}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="config-main">
        {document === null ? (
          <div className="empty tall">选择一个配置文件，或导入文件开始编辑</div>
        ) : (
          <>
            <div className="config-document-head">
              <label htmlFor="configuration-name">配置名</label>
              <input
                id="configuration-name"
                className="mono"
                value={document.name}
                disabled={document.source === "stored"}
                onChange={(event) =>
                  setDocument(configuration.rename(document, event.target.value))
                }
                spellCheck={false}
              />
              {document.source === "imported" && !isConfigurationName(document.name) && (
                <span className="danger-text">名称需符合 *.conf 规则</span>
              )}
            </div>
            <textarea
              className="conf-editor"
              value={document.text}
              onChange={(event) =>
                setDocument(configuration.edit(document, event.target.value))
              }
              spellCheck={false}
            />
          </>
        )}

        {error && <div className="err-box">{error}</div>}

        <div className="toolbar bottom">
          {document && (
            <>
              <button
                className="btn primary"
                onClick={() => save(false)}
                disabled={busy !== null || !isConfigurationName(document.name)}
              >
                {busy === "save"
                  ? "保存中…"
                  : document.source === "imported"
                    ? "保存为新配置"
                    : "保存"}
              </button>
              <button
                className="btn"
                onClick={() => save(true)}
                disabled={busy !== null || !isConfigurationName(document.name)}
              >
                {busy === "apply" ? "应用中…" : "保存并热同步"}
              </button>
              {document.source === "stored" && (
                <button className="btn ghost" onClick={exportFile} disabled={busy !== null}>
                  导出
                </button>
              )}
              <span className="muted small">
                {document.dirty ? "● 未保存" : "已保存"} · 原文保存 · 权限 0600
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
