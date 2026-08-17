import { describe, expect, it } from "vitest";
import { renderConfigurationQr } from "./configurationQr";

const CLIENT_CONFIGURATION = `[Interface]
PrivateKey = client-private-key
Address = 10.66.66.2/32

[Peer]
PublicKey = server-public-key
Endpoint = vpn.example.net:51820
AllowedIPs = 0.0.0.0/0
`;

describe("renderConfigurationQr", () => {
  it("renders a WireGuard configuration as an SVG data URL", async () => {
    const result = await renderConfigurationQr(CLIENT_CONFIGURATION);

    expect(result).toMatch(/^data:image\/svg\+xml;charset=utf-8,/);
    expect(decodeURIComponent(result)).toContain("<svg");
  });

  it("rejects empty configurations", async () => {
    await expect(renderConfigurationQr(" \n ")).rejects.toThrow("配置内容为空");
  });

  it("reports configurations that exceed QR capacity", async () => {
    await expect(renderConfigurationQr("x".repeat(2954))).rejects.toThrow(
      "配置超过二维码容量",
    );
  });
});
