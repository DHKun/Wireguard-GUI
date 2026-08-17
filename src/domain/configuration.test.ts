import { describe, expect, it } from "vitest";
import { ConfigurationLifecycle, isConfigurationName } from "./configuration";
import { InMemoryWireGuard } from "../test/InMemoryWireGuard";

describe("ConfigurationLifecycle", () => {
  it("turns an imported file into an editable named document", () => {
    const lifecycle = new ConfigurationLifecycle(new InMemoryWireGuard());

    const document = lifecycle.imported("office tunnel.conf", "[Interface]\n");

    expect(document).toEqual({
      name: "office-tunnel.conf",
      text: "[Interface]\n",
      source: "imported",
      dirty: true,
    });
  });

  it("keeps persistence success visible when runtime sync warns", async () => {
    const port = new InMemoryWireGuard();
    port.configurationOutcome = {
      persisted: true,
      runtime_applied: false,
      warnings: ["配置已保存，热同步失败"],
    };
    const lifecycle = new ConfigurationLifecycle(port);
    const document = lifecycle.imported("wg0.conf", "[Interface]\n");

    const result = await lifecycle.apply(document, true);

    expect(result.document).toMatchObject({ source: "stored", dirty: false });
    expect(result.message).toContain("配置已保存，热同步失败");
    expect(port.configurations.get("wg0.conf")).toBe("[Interface]\n");
  });

  it("rejects traversal and malformed names", () => {
    expect(isConfigurationName("wg0.conf")).toBe(true);
    expect(isConfigurationName("../wg0.conf")).toBe(false);
    expect(isConfigurationName(".hidden.conf")).toBe(false);
    expect(isConfigurationName("wg0")).toBe(false);
  });
});
