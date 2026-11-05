import { describe, expect, it } from "vitest";
import i18n from "@/shared/i18n";
import { opKey } from "./ActivityItem";

describe("opKey", () => {
  it("traduce también las operaciones de tres niveles", () => {
    expect(opKey("container.list")).toBe("ops.container.list");
    expect(opKey("container.exec.create")).toBe("ops.container.exec_create");
    for (const k of ["container.create", "container.inspect", "container.logs", "container.stats", "container.exec", "container.exec.create", "container.exec.inspect", "image.inspect", "network.list", "volume.list"]) {
      expect(i18n.exists(opKey(k)), k).toBe(true);
    }
  });
});
