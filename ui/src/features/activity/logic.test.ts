// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { apiRecord, cliRecord } from "@/test/fixtures";
import { EMPTY_FILTER, describeTransport, formatDuration, headlineCommand, matches, quoteArg, renderArgv, upsert, isFiltered, recordServer } from "./logic";

describe("quoteArg (misma regla que Rust)", () => {
  it("no entrecomilla argumentos seguros", () => {
    expect(renderArgv("docker", ["run", "-d", "--name", "web", "-p", "8080:80", "nginx:1.27"])).toBe(
      "docker run -d --name web -p 8080:80 nginx:1.27",
    );
  });
  it("entrecomilla metacaracteres del shell", () => {
    expect(quoteArg("hola mundo")).toBe("'hola mundo'");
    expect(quoteArg("a;rm -rf /")).toBe("'a;rm -rf /'");
    expect(quoteArg("$(whoami)")).toBe("'$(whoami)'");
    expect(quoteArg("`id`")).toBe("'`id`'");
    expect(quoteArg("a|b&c>d")).toBe("'a|b&c>d'");
  });
  it("escapa comillas simples", () => {
    expect(quoteArg("it's")).toBe("'it'\\''s'");
  });
  it("conserva argumentos vacíos y entrecomilla no ASCII", () => {
    expect(renderArgv("docker", ["exec", ""])).toBe("docker exec ''");
    expect(quoteArg("señal")).toBe("'señal'");
  });
});

describe("describeTransport / headlineCommand", () => {
  it("API muestra método y ruta; titular es el equivalente", () => {
    const r = apiRecord();
    expect(describeTransport(r.transport)).toBe("POST /v1.47/containers/web/start");
    expect(headlineCommand(r)).toBe("docker start web");
  });
  it("CLI muestra el comando realmente ejecutado", () => {
    const r = cliRecord();
    expect(headlineCommand(r)).toBe("docker compose -f /srv/blog/compose.yaml up -d");
  });
  it("API sin equivalente devuelve null", () => {
    expect(headlineCommand(apiRecord({ equivalentCommand: null }))).toBeNull();
  });
});

describe("matches", () => {
  const recs = [
    apiRecord({ kind: "container.start", target: "web" }),
    apiRecord({ kind: "image.pull", category: "image", target: "nginx:1.27", transport: { via: "api", method: "POST", path: "/v1.47/images/create?fromImage=nginx&tag=1.27", endpoint: "/var/run/docker.sock" }, status: "failed", error: "manifest unknown", equivalentCommand: "docker pull nginx:1.27" }),
    cliRecord(),
  ];
  const run = (f: Partial<typeof EMPTY_FILTER>) => recs.filter((r) => matches(r, { ...EMPTY_FILTER, ...f })).map((r) => r.kind);

  it("sin filtros devuelve todo", () => expect(run({})).toHaveLength(3));
  it("filtra por categoría", () => expect(run({ category: "image" })).toEqual(["image.pull"]));
  it("filtra por estado", () => expect(run({ status: "failed" })).toEqual(["image.pull"]));
  it("busca en comandos, errores y destino, sin distinguir mayúsculas", () => {
    expect(run({ query: "MANIFEST" })).toEqual(["image.pull"]);
    expect(run({ query: "compose up" })).toEqual(["compose.up"]);
    expect(run({ query: "docker start" })).toEqual(["container.start"]);
  });
  it("todas las palabras deben aparecer", () => expect(run({ query: "docker nada" })).toEqual([]));
});

describe("upsert", () => {
  it("añade arriba y respeta el máximo", () => {
    const a = apiRecord();
    const b = apiRecord();
    const c = apiRecord();
    expect(upsert(upsert(upsert([], a, 2), b, 2), c, 2).map((r) => r.id)).toEqual([c.id, b.id]);
  });
  it("reemplaza en su sitio", () => {
    const a = apiRecord({ status: "running" });
    const b = apiRecord();
    const list = [b, a];
    const next = upsert(list, { ...a, status: "succeeded" }, 10);
    expect(next.map((r) => r.id)).toEqual([b.id, a.id]);
    expect(next[1]!.status).toBe("succeeded");
  });
});

describe("formatDuration", () => {
  it("formatea en ms, s y min", () => {
    expect(formatDuration(null)).toBe("");
    expect(formatDuration(87)).toBe("87 ms");
    expect(formatDuration(1530)).toBe("1.53 s");
    expect(formatDuration(42_100)).toBe("42.1 s");
    expect(formatDuration(125_000)).toBe("2 min 5 s");
  });
});

describe("filtro por servidor (0.10.0)", () => {
  const remote = { id: "srv-0123abcd", name: "Producción", address: "deploy@docker-a:22", remote: true };
  it("filtra por servidor; los registros sin servidor son del equipo local", () => {
    const a = apiRecord({ server: remote });
    const old = apiRecord({ server: null });
    const f = { ...EMPTY_FILTER, server: "srv-0123abcd" };
    expect(matches(a, f)).toBe(true);
    expect(matches(old, f)).toBe(false);
    expect(matches(old, { ...EMPTY_FILTER, server: "local" })).toBe(true);
    expect(matches(a, { ...EMPTY_FILTER, server: "local" })).toBe(false);
    expect(isFiltered({ ...EMPTY_FILTER, server: "local" })).toBe(true);
    expect(recordServer(old)).toBe("local");
  });
  it("el nombre del servidor se puede buscar", () => {
    expect(matches(apiRecord({ server: remote }), { ...EMPTY_FILTER, query: "producción" })).toBe(true);
  });
});

describe("filtro por operación masiva (1.10.0)", () => {
  it("muestra solo los registros de un lote y cuenta como filtro", () => {
    const batch = "0f0e0d0c-0b0a-4908-8706-050403020100";
    const inBatch = apiRecord({ batch });
    const other = apiRecord({ batch: "11111111-0b0a-4908-8706-050403020100" });
    const plain = apiRecord();
    const f = { ...EMPTY_FILTER, batch };
    expect(matches(inBatch, f)).toBe(true);
    expect(matches(other, f)).toBe(false);
    expect(matches(plain, f)).toBe(false);
    expect(matches(plain, EMPTY_FILTER)).toBe(true);
    expect(isFiltered(f)).toBe(true);
  });
});
