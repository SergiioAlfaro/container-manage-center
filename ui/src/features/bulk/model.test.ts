// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { bulkContainer, bulkInventory, fullId } from "@/test/apiMock";
import type { BulkOperation } from "@/shared/ipc/bindings/BulkOperation";
import {
  EMPTY_FILTER,
  applicableCount,
  envsOf,
  imagesOf,
  isFiltered,
  kpis,
  matches,
  matchesLabel,
  pageCount,
  pageOf,
  plan,
  policyText,
  pruneSelection,
  relevantLabels,
  rowKey,
  rowsOf,
  selectAll,
  skipReason,
  sortRows,
  targetsOf,
  validPolicy,
} from "./model";

const rows = () => rowsOf(bulkInventory(), "Este equipo");
const names = (rs: ReturnType<typeof rows>) => rs.map((r) => r.c.name);

describe("modelo de la gestión masiva", () => {
  it("reúne los contenedores de todos los servidores; el mismo ID en dos servidores son dos filas", () => {
    const rs = rows();
    expect(names(rs)).toEqual(["web", "db", "cache", "api", "tarea"]);
    const web = rs.find((r) => r.c.name === "web")!;
    const api = rs.find((r) => r.c.name === "api")!;
    expect(web.c.id).toBe(api.c.id);
    expect(web.key).not.toBe(api.key);
    expect(web.key).toBe(rowKey("local", fullId("a")));
    expect(web.envName).toBe("Este equipo");
    expect(api.envName).toBe("Producción");
    expect(rowsOf(undefined, "x")).toEqual([]);
  });

  it("cuenta total, en ejecución, detenidos, en pausa y con error", () => {
    expect(kpis(rows())).toEqual({ total: 5, running: 3, stopped: 1, paused: 1, error: 1 });
  });

  it("filtra por nombre, prefijo de ID, estado, imagen, etiqueta y servidor", () => {
    const rs = rows();
    const run = (f: Partial<typeof EMPTY_FILTER>) => names(rs.filter((r) => matches(r, { ...EMPTY_FILTER, ...f })));
    expect(run({})).toHaveLength(5);
    expect(run({ query: "WE" })).toEqual(["web"]);
    expect(run({ query: "dddd" })).toEqual(["tarea"]);
    expect(run({ state: "running" })).toEqual(["web", "api", "tarea"]);
    expect(run({ state: "stopped" })).toEqual(["db"]);
    expect(run({ state: "paused" })).toEqual(["cache"]);
    expect(run({ state: "error" })).toEqual(["db"]);
    expect(run({ image: "nginx:1.27" })).toEqual(["web", "api"]);
    expect(run({ label: "tier" })).toEqual(["web", "db", "api"]);
    expect(run({ label: "tier=backend" })).toEqual(["db"]);
    expect(run({ label: " tier = frontend " })).toEqual(["web", "api"]);
    expect(run({ env: "srv-prod" })).toEqual(["api", "tarea"]);
    expect(run({ env: "srv-prod", state: "running", image: "alpine:3" })).toEqual(["tarea"]);
    expect(isFiltered(EMPTY_FILTER)).toBe(false);
    expect(isFiltered({ ...EMPTY_FILTER, label: "x" })).toBe(true);
    expect(matchesLabel({ a: "" }, "a=")).toBe(true);
    expect(matchesLabel({}, "toString")).toBe(false);
  });

  it("lista imágenes distintas y solo las etiquetas propias", () => {
    expect(imagesOf(rows())).toEqual(["alpine:3", "nginx:1.27", "postgres:16", "redis:7"]);
    expect(relevantLabels({ "com.docker.compose.project": "x", "org.opencontainers.image.title": "y", tier: "a", app: "b" })).toEqual([
      ["app", "b"],
      ["tier", "a"],
    ]);
  });

  it("ordena por cada columna en los dos sentidos, con desempate estable", () => {
    const rs = rows();
    expect(names(sortRows(rs, { key: "name", dir: "asc" }))).toEqual(["api", "cache", "db", "tarea", "web"]);
    expect(names(sortRows(rs, { key: "name", dir: "desc" }))).toEqual(["web", "tarea", "db", "cache", "api"]);
    expect(names(sortRows(rs, { key: "env", dir: "asc" })).slice(0, 3)).toEqual(["web", "db", "cache"]);
    expect(names(sortRows(rs, { key: "restart", dir: "asc" }))[0]).toBe("cache");
    const created = rs.map((r, i) => ({ ...r, c: { ...r.c, created: 100 - i } }));
    expect(names(sortRows(created, { key: "created", dir: "desc" }))).toEqual(["web", "db", "cache", "api", "tarea"]);
    // No cambia la lista original.
    expect(names(rs)).toEqual(["web", "db", "cache", "api", "tarea"]);
  });

  it("pagina sin salirse del rango", () => {
    const xs = Array.from({ length: 120 }, (_, i) => i);
    expect(pageCount(120, 50)).toBe(3);
    expect(pageCount(0, 50)).toBe(1);
    expect(pageOf(xs, 2, 50)).toEqual(xs.slice(100));
    expect(pageOf(xs, 9, 50)).toEqual(xs.slice(100));
  });

  it("selección: todo lo filtrado y limpieza de lo que ya no existe", () => {
    const rs = rows();
    const sel = selectAll(new Set(), rs.filter((r) => r.server === "srv-prod"));
    expect(sel.size).toBe(2);
    const pruned = pruneSelection(new Set([...sel, "local\u0000zzz"]), rs);
    expect(pruned).toEqual(sel);
  });

  it("aplica las mismas reglas que el motor y explica cada omisión", () => {
    const st = (state: string) => bulkContainer({ state });
    const op = (o: BulkOperation) => o;
    expect(skipReason(op({ kind: "start" }), st("exited"))).toBeNull();
    expect(skipReason(op({ kind: "start" }), st("running"))).toEqual({ reason: "state", state: "running" });
    expect(skipReason(op({ kind: "stop" }), st("paused"))).toBeNull();
    expect(skipReason(op({ kind: "stop" }), st("exited"))).toEqual({ reason: "state", state: "exited" });
    expect(skipReason(op({ kind: "pause" }), st("paused"))).not.toBeNull();
    expect(skipReason(op({ kind: "unpause" }), st("paused"))).toBeNull();
    expect(skipReason(op({ kind: "restart" }), st("running"))).toBeNull();
    expect(skipReason(op({ kind: "remove", force: false }), st("running"))).toEqual({ reason: "needsForce", state: "running" });
    expect(skipReason(op({ kind: "remove", force: true }), st("running"))).toBeNull();
    expect(skipReason(op({ kind: "remove", force: true }), st("removing"))).toEqual({ reason: "state", state: "removing" });
    const policy = (name: "always" | "unless-stopped") => op({ kind: "restartPolicy", policy: { name, maximumRetryCount: 0 } });
    expect(skipReason(policy("unless-stopped"), bulkContainer())).toEqual({ reason: "unchanged" });
    expect(skipReason(policy("always"), bulkContainer())).toBeNull();
    expect(skipReason(policy("always"), bulkContainer({ details: { restartPolicy: { name: "no", maximumRetryCount: 0 }, autoRemove: true } }))).toEqual({ reason: "autoRemove" });
    // Sin datos de inspect se envía: el motor lo comprueba con el estado real.
    expect(skipReason(policy("always"), bulkContainer({ details: null }))).toBeNull();
  });

  it("separa lo que se envía de lo que se omite y conserva servidor e ID completo", () => {
    const rs = rows();
    const p = plan({ kind: "stop" }, rs);
    expect(names(p.apply)).toEqual(["web", "cache", "api", "tarea"]);
    expect(p.skip.map((s) => s.row.c.name)).toEqual(["db"]);
    const ts = targetsOf(p.apply);
    expect(ts[0]).toEqual({ server: "local", id: fullId("a"), name: "web" });
    expect(ts[2]).toEqual({ server: "srv-prod", id: fullId("a"), name: "api" });
    expect(applicableCount("start", rs)).toBe(1);
    expect(applicableCount("unpause", rs)).toBe(1);
    expect(envsOf(p.apply)).toEqual([
      { server: "local", name: "Este equipo", remote: false, count: 2 },
      { server: "srv-prod", name: "Producción", remote: true, count: 2 },
    ]);
  });

  it("política de reinicio: texto y validación iguales que en Rust", () => {
    expect(policyText({ name: "on-failure", maximumRetryCount: 3 })).toBe("on-failure:3");
    expect(policyText({ name: "on-failure", maximumRetryCount: 0 })).toBe("on-failure");
    expect(policyText(null)).toBe("");
    expect(validPolicy({ name: "on-failure", maximumRetryCount: 5 })).toBe(true);
    expect(validPolicy({ name: "always", maximumRetryCount: 3 })).toBe(false);
    expect(validPolicy({ name: "on-failure", maximumRetryCount: 1001 })).toBe(false);
    expect(validPolicy({ name: "on-failure", maximumRetryCount: 1.5 })).toBe(false);
  });
});
