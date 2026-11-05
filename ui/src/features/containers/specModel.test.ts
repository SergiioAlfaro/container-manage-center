import { describe, expect, it } from "vitest";
import { bytesFrom, isSecretKey, issuesFor, tabOf, toMemField } from "./specModel";

describe("specModel", () => {
  it("asigna cada campo a su pestaña", () => {
    expect(tabOf("image")).toBe("general");
    expect(tabOf("env.3.key")).toBe("env");
    expect(tabOf("ports.0.hostPort")).toBe("network");
    expect(tabOf("mounts.1.source")).toBe("storage");
    expect(tabOf("memoryBytes")).toBe("resources");
    expect(tabOf("capAdd")).toBe("advanced");
    expect(tabOf("autoRemove")).toBe("advanced");
  });
  it("filtra problemas por campo o por grupo", () => {
    const iss = [
      { severity: "error" as const, field: "ports.0.hostPort", code: "port.invalid", params: {} },
      { severity: "warning" as const, field: "ports", code: "ports.ignoredWithNetwork", params: {} },
    ];
    expect(issuesFor(iss, "ports.0.hostPort")).toHaveLength(1);
    expect(issuesFor(iss, "ports", false)).toHaveLength(2);
  });
  it("reconoce claves con secretos", () => {
    expect(isSecretKey("POSTGRES_PASSWORD")).toBe(true);
    expect(isSecretKey("api_token")).toBe(true);
    expect(isSecretKey("PATH")).toBe(false);
  });
  it("convierte memoria entre bytes y campo", () => {
    expect(bytesFrom("512", "m")).toBe(512 * 1024 ** 2);
    expect(bytesFrom("1,5", "g")).toBe(1.5 * 1024 ** 3);
    expect(bytesFrom("", "m")).toBeNull();
    expect(bytesFrom("abc", "m")).toBeNaN();
    expect(toMemField(2 * 1024 ** 3)).toEqual({ value: "2", unit: "g" });
    expect(toMemField(256 * 1024 ** 2)).toEqual({ value: "256", unit: "m" });
  });
});
