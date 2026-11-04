import { describe, expect, it } from "vitest";
import { formatBytes, formatPort, formatRelative, shortId, uniquePorts } from "./format";

describe("format", () => {
  it("bytes en unidades binarias", () => {
    expect(formatBytes(0, "es")).toBe("0 B");
    expect(formatBytes(1536, "en")).toBe("1.5 KiB");
    expect(formatBytes(192_000_000, "es")).toBe("183 MiB");
    expect(formatBytes(null, "es")).toBe("");
  });
  it("IDs cortos sin prefijo", () => {
    expect(shortId("sha256:0123456789abcdef")).toBe("0123456789ab");
    expect(shortId("abcdef")).toBe("abcdef");
  });
  it("puertos", () => {
    expect(formatPort({ ip: "0.0.0.0", privatePort: 80, publicPort: 8080, protocol: "tcp" })).toBe("8080→80/tcp");
    expect(formatPort({ ip: "127.0.0.1", privatePort: 5432, publicPort: 5432, protocol: "tcp" })).toBe("127.0.0.1:5432→5432/tcp");
    expect(formatPort({ ip: null, privatePort: 53, publicPort: null, protocol: "udp" })).toBe("53/udp");
    expect(
      uniquePorts([
        { ip: "0.0.0.0", privatePort: 80, publicPort: 8080, protocol: "tcp" },
        { ip: "::", privatePort: 80, publicPort: 8080, protocol: "tcp" },
      ]),
    ).toEqual(["8080→80/tcp"]);
  });
  it("tiempo relativo", () => {
    const now = Date.UTC(2026, 10, 4, 12);
    expect(formatRelative(now / 1000 - 7200, "es", now)).toBe("hace 2 horas");
    expect(formatRelative(now / 1000 - 30, "en", now)).toBe("30 seconds ago");
  });
});
