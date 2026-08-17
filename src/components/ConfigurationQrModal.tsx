import { useEffect, useId, useState } from "react";
import { renderConfigurationQr } from "../domain/configurationQr";
import { errText } from "../wireguard";

interface Props {
  name: string;
  configuration: string;
  onClose: () => void;
}

export default function ConfigurationQrModal({ name, configuration, onClose }: Props) {
  const titleId = useId();
  const [source, setSource] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setSource(null);
    setError(null);
    void renderConfigurationQr(configuration)
      .then((result) => active && setSource(result))
      .catch((cause) => active && setError(errText(cause)));
    return () => {
      active = false;
    };
  }, [configuration]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <div
        className="modal qr-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <h3 id={titleId}>配置二维码</h3>
        <div className="qr-document-name mono">{name}</div>
        <div className="qr-warning">
          二维码包含配置中的私钥和预共享密钥。请仅使用可信设备扫描。
        </div>

        <div className="qr-preview">
          {error ? (
            <div className="err-box">{error}</div>
          ) : source ? (
            <img src={source} alt={`${name} 配置二维码`} />
          ) : (
            <span className="muted">正在生成二维码…</span>
          )}
        </div>

        <div className="modal-actions">
          <button className="btn primary" onClick={onClose} autoFocus>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
