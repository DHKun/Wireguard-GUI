import QRCode from "qrcode";

const MAX_QR_BYTES = 2953;

export async function renderConfigurationQr(configuration: string): Promise<string> {
  if (configuration.trim() === "") {
    throw new Error("配置内容为空，无法生成二维码");
  }

  if (new TextEncoder().encode(configuration).length > MAX_QR_BYTES) {
    throw new Error("配置超过二维码容量，请减少 Peer 数量后重试");
  }

  const svg = await QRCode.toString(configuration, {
    type: "svg",
    errorCorrectionLevel: "L",
    margin: 2,
    width: 360,
    color: {
      dark: "#0f1115",
      light: "#ffffff",
    },
  });

  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
