import { useEffect, useState } from "react";
import { api, errText } from "../api";

interface Props {
  notify: (msg: string, tone: "ok" | "err") => void;
}

export default function ConfigPanel({ notify }: Props) {
  const [configs, setConfigs] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      const list = await api.listConfigs();
      setConfigs(list);
      if (selected && !list.includes(selected)) {
        setSelected(null);
        setText("");
        setDirty(false);
      }
    } catch (e) {
      setError(errText(e));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function open(name: string) {
    if (dirty && !window.confirm(`放弃对 ${selected} 的未保存修改？`)) return;
    setBusy("load");
    setError(null);
    try {
      const t = await api.readConfig(name);
      setSelected(name);
      setText(t);
      setDirty(false);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
    }
  }

  async function save(apply: boolean) {
    if (!selected) return;
    setBusy(apply ? "apply" : "save");
    setError(null);
    try {
      await api.writeConfig(selected, text);
      if (apply) {
        await api.syncconf(selected.replace(/\.conf$/, ""));
      }
      setDirty(false);
      notify(apply ? `已保存并热同步 ${selected}` : `已保存 ${selected}`, "ok");
    } catch (e) {
      setError(errText(e));
      notify(`保存失败：${errText(e)}`, "err");
    } finally {
      setBusy(null);
    }
  }

  async function importFile() {
    // 浏览器级文件选择器（webkit2gtk 原生对话框）
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".conf,text/plain";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        setText(await file.text());
        setSelected(null);
        setDirty(true);
        notify(`已载入 ${file.name} 到编辑器（尚未写入）`, "ok");
      } catch (e) {
        notify(`读取文件失败：${errText(e)}`, "err");
      }
    };
    input.click();
  }

  async function exportFile() {
    if (!selected) return;
    setBusy("export");
    try {
      const env = await api.checkEnv();
      const home = env.home || "/home/dohokun";
      const dest = window.prompt(
        `导出 ${selected} 到（绝对路径）：`,
        `${home}/Downloads/${selected}`,
      );
      if (!dest) return;
      await api.exportConfig(selected, dest);
      notify(`已导出到 ${dest}`, "ok");
    } catch (e) {
      notify(`导出失败：${errText(e)}`, "err");
    } finally {
      setBusy(null);
    }
  }

  const ifaceName = selected?.replace(/\.conf$/, "");

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
            {configs.map((c) => (
              <li key={c}>
                <button
                  className={`config-item mono ${selected === c ? "active" : ""}`}
                  onClick={() => open(c)}
                >
                  {c}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="config-main">
        {selected === null ? (
          <div className="empty tall">
            {text === "" ? "选择一个配置文件，或导入文件开始编辑" : "（新文件）输入内容后点「保存为新文件」"}
          </div>
        ) : (
          <textarea
            className="conf-editor"
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setDirty(true);
            }}
            spellCheck={false}
          />
        )}

        {error && <div className="err-box">{error}</div>}

        <div className="toolbar bottom">
          {selected && (
            <>
              <button className="btn primary" onClick={() => save(false)} disabled={busy !== null}>
                {busy === "save" ? "保存中…" : "保存"}
              </button>
              <button className="btn" onClick={() => save(true)} disabled={busy !== null || !ifaceName}>
                {busy === "apply" ? "应用中…" : "保存并热同步"}
              </button>
              <button className="btn ghost" onClick={exportFile} disabled={busy !== null}>
                导出
              </button>
              <span className="muted small">
                {dirty ? "● 未保存" : "已保存"} · 写入时权限 0600，注释不保留
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
