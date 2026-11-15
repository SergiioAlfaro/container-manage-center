// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";

// Los tipos generados por ts-rs deben describir lo que serde envía de verdad.
const files = import.meta.glob("./bindings/*.ts", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

describe("tipos IPC generados", () => {
  it("existen", () => {
    expect(Object.keys(files).length).toBeGreaterThan(30);
  });
  it("no usan bigint: serde serializa los enteros como números JSON", () => {
    const offenders = Object.entries(files).filter(([, src]) => /\bbigint\b/.test(src)).map(([f]) => f);
    expect(offenders).toEqual([]);
  });
  it("no exponen campos en snake_case", () => {
    const offenders = Object.entries(files)
      .filter(([, src]) => /[{,]\s*[a-z]+_[a-z_]+\s*:/.test(src.replace(/\/\*[\s\S]*?\*\//g, "")))
      .map(([f]) => f);
    expect(offenders).toEqual([]);
  });
});
