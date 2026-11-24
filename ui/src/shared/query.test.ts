// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { keys } from "./query";

const SRC = join(__dirname, "..");

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return name === "bindings" ? [] : sources(p);
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [p] : [];
  });
}

describe("claves de la caché de consultas (1.11.1)", () => {
  it("no hay dos claves iguales en keys", () => {
    const all = Object.values(keys).map((k) => JSON.stringify(k));
    expect(new Set(all).size).toBe(all.length);
  });

  it("cada clave fija se pide siempre con la misma función (las listas breves no pisan a las completas)", () => {
    // Antes de la 1.11.1, el asistente de contenedores guardaba networksBrief y
    // volumesBrief en ["networks"] y ["volumes"]: después, Redes y Copias
    // leían esa lista breve y la interfaz fallaba.
    const keyOf: Record<string, string> = Object.fromEntries(Object.entries(keys).map(([n, k]) => [`keys.${n}`, JSON.stringify(k)]));
    const seen = new Map<string, Set<string>>();
    const re = /queryKey:\s*(keys\.\w+|\[[^\]\n]*\])\s*,\s*queryFn:\s*(?:api|bound)\.(\w+)\b/g;
    for (const file of sources(SRC)) {
      const text = readFileSync(file, "utf8");
      for (const m of text.matchAll(re)) {
        const raw = m[1]!;
        // Solo claves fijas (sin variables dentro).
        const key = raw.startsWith("keys.") ? keyOf[raw] : /^\[(\s*"[^"]*"\s*,?)*\]$/.test(raw) ? JSON.stringify(JSON.parse(raw)) : undefined;
        if (!key) continue;
        const fns = seen.get(key) ?? new Set<string>();
        fns.add(`${m[2]} (${relative(SRC, file)})`);
        seen.set(key, fns);
      }
    }
    expect(seen.size).toBeGreaterThan(10);
    const clashes = [...seen].filter(([, fns]) => new Set([...fns].map((f) => f.split(" ")[0])).size > 1);
    expect(clashes.map(([k, fns]) => `${k}: ${[...fns].join(", ")}`)).toEqual([]);
  });
});
