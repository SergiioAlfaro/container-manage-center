import { describe, expect, it } from "vitest";
import es from "./locales/es.json";
import en from "./locales/en.json";

function keys(obj: unknown, prefix = ""): string[] {
  if (obj === null || typeof obj !== "object" || Array.isArray(obj)) return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) => keys(v, prefix ? `${prefix}.${k}` : k));
}

describe("traducciones", () => {
  it("español e inglés tienen exactamente las mismas claves", () => {
    expect(keys(en).sort()).toEqual(keys(es).sort());
  });
  it("ninguna traducción está vacía", () => {
    for (const lang of [es, en]) {
      for (const k of keys(lang)) {
        const v = k.split(".").reduce<unknown>((o, p) => (o as Record<string, unknown>)?.[p], lang);
        if (typeof v === "string") expect(v.trim(), k).not.toBe("");
      }
    }
  });
});
