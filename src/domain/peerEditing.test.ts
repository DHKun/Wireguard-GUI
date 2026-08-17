import { describe, expect, it } from "vitest";
import { PeerEditing } from "./peerEditing";
import { InMemoryWireGuard } from "../test/InMemoryWireGuard";

const key = "A".repeat(44);

describe("PeerEditing", () => {
  it("updates drafts immutably and preserves extras", () => {
    const editing = new PeerEditing(new InMemoryWireGuard());
    const original = [
      {
        public_key: key,
        allowed_ips: ["10.0.0.2/32"],
        extras: [["Custom", "keep"]] as [string, string][],
      },
    ];

    const next = editing.save(original, 0, {
      ...original[0],
      allowed_ips: ["10.0.0.3/32"],
    });

    expect(original[0].allowed_ips).toEqual(["10.0.0.2/32"]);
    expect(next[0].allowed_ips).toEqual(["10.0.0.3/32"]);
    expect(next[0].extras).toEqual([["Custom", "keep"]]);
  });

  it("rejects duplicate public keys before crossing the seam", async () => {
    const port = new InMemoryWireGuard();
    const editing = new PeerEditing(port);
    const duplicate = { public_key: key, allowed_ips: ["10.0.0.2/32"], extras: [] };

    await expect(
      editing.apply("wg0", [duplicate, { ...duplicate }], "runtime_only"),
    ).rejects.toThrow("Peer 公钥重复");
    expect(port.appliedPeers).toHaveLength(0);
  });

  it("passes Apply Mode and normalized peers through one interface", async () => {
    const port = new InMemoryWireGuard();
    const editing = new PeerEditing(port);

    await editing.apply(
      "wg0",
      [{ public_key: ` ${key} `, allowed_ips: [" 10.0.0.2/32 "], extras: [] }],
      "persist_and_sync",
    );

    expect(port.appliedPeers[0]).toMatchObject({
      name: "wg0",
      mode: "persist_and_sync",
      peers: [{ public_key: key, allowed_ips: ["10.0.0.2/32"] }],
    });
  });
});
