// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { bulkInventory } from "@/test/apiMock";
import type { ExecItemResult } from "@/shared/ipc/bindings/ExecItemResult";
import { CUSTOM, EMPTY_FORM, EXAMPLES, argsOf, commandOf, exportJson, exportMarkdown, formFromTemplate, formReady, shownStatus, splitTargets, templateFromForm } from "./execModel";
import { rowsOf } from "./model";

const t = (k: string, o?: Record<string, unknown>) => (o ? `${k}${JSON.stringify(o)}` : k);

function result(over: Partial<ExecItemResult> = {}): ExecItemResult {
  return {
    index: 0,
    target: { server: "local", id: "a".repeat(64), name: "web" },
    status: "succeeded",
    output: { end: "exited", exitCode: 0, stillRunning: false, stdout: "DB_PASSWORD=S3creta\nok\n", stderr: "", stdoutTruncated: false, stderrTruncated: true, launchFailed: false, startedAt: "2026-11-20T10:00:00Z" },
    skip: null,
    cancelled: false,
    error: null,
    stage: null,
    durationMs: 42,
    ...over,
  };
}

describe("modelo de los comandos masivos", () => {
  it("argumentos: uno por línea, con espacios, sin interpretar nada", () => {
    expect(argsOf("")).toEqual([]);
    expect(argsOf("-h\n")).toEqual(["-h"]);
    expect(argsOf("a b\n$(id)\n; rm -rf /")).toEqual(["a b", "$(id)", "; rm -rf /"]);
    expect(argsOf("a\n\nb")).toEqual(["a", "", "b"]);
  });

  it("del formulario al comando del backend en las dos modalidades", () => {
    expect(commandOf({ ...EMPTY_FORM, program: " df ", argsText: "-h", user: " ", workdir: "/srv", timeoutSecs: 10 })).toEqual({
      command: { mode: "direct", program: "df", args: ["-h"] },
      user: null,
      workdir: "/srv",
      timeoutSecs: 10,
    });
    const s = commandOf({ ...EMPTY_FORM, mode: "script", interpreter: CUSTOM, customInterpreter: " /usr/bin/perl ", flag: "-e", script: "print 1\nprint 2", user: "app" });
    expect(s.command).toEqual({ mode: "script", interpreter: "/usr/bin/perl", flag: "-e", script: "print 1\nprint 2" });
    expect(s.user).toBe("app");
  });

  it("el formulario está listo solo con lo imprescindible y un tiempo válido", () => {
    expect(formReady(EMPTY_FORM)).toBe(false);
    expect(formReady({ ...EMPTY_FORM, program: "id" })).toBe(true);
    expect(formReady({ ...EMPTY_FORM, program: "id", timeoutSecs: 0 })).toBe(false);
    expect(formReady({ ...EMPTY_FORM, program: "id", timeoutSecs: 3601 })).toBe(false);
    expect(formReady({ ...EMPTY_FORM, mode: "script", script: "  " })).toBe(false);
    expect(formReady({ ...EMPTY_FORM, mode: "script", interpreter: CUSTOM, customInterpreter: "", script: "id" })).toBe(false);
  });

  it("plantillas: ida y vuelta sin perder nada, y ejemplos sin secretos", () => {
    const f = { ...EMPTY_FORM, mode: "script" as const, interpreter: "/bin/bash", script: "df -h\nfree", user: "root", workdir: "/tmp", timeoutSecs: 60 };
    const tp = templateFromForm(f, "id1", " Recursos ", " desc ");
    expect(tp).toMatchObject({ id: "id1", name: "Recursos", description: "desc", mode: "script", interpreter: "/bin/bash", flag: "-c", program: "", args: [] });
    expect(formFromTemplate(tp)).toEqual(f);
    const custom = formFromTemplate({ ...tp, interpreter: "/opt/x/sh" });
    expect(custom.interpreter).toBe(CUSTOM);
    expect(custom.customInterpreter).toBe("/opt/x/sh");
    expect(EXAMPLES.map((e) => e.key)).toEqual(["osRelease", "disk", "memory", "processes", "packagesIndex", "network", "environment"]);
    for (const e of EXAMPLES) expect(formReady(formFromTemplate(e.template)), e.key).toBe(true);
  });

  it("solo se ejecuta en contenedores en ejecución", () => {
    const rows = rowsOf(bulkInventory(), "Este equipo");
    const { run, skip } = splitTargets(rows);
    expect(run.map((r) => r.c.name)).toEqual(["web", "api", "tarea"]);
    expect(skip.map((r) => r.c.name)).toEqual(["db", "cache"]);
  });

  it("estado que se muestra antes y después del resultado", () => {
    expect(shownStatus(undefined, false)).toBe("pending");
    expect(shownStatus(undefined, true)).toBe("running");
    expect(shownStatus(result({ status: "timedOut" }), false)).toBe("timedOut");
  });

  it("exportaciones con la salida de cada contenedor y los secretos ocultos", () => {
    const meta = { rows: [{ envName: "Este equipo", image: "nginx:1.27" }], command: "docker exec '<contenedor>' env", user: null, workdir: null, timeoutSecs: 30, batch: "b1", startedAt: null, durationMs: 50 };
    const md = exportMarkdown(t, [result()], meta);
    expect(md).toContain("## web");
    expect(md).toContain("nginx:1.27");
    expect(md).toContain("DB_PASSWORD=********");
    expect(md).not.toContain("S3creta");
    expect(md).toContain("bulk.exec.truncated");
    const json = JSON.parse(exportJson([result({ status: "failed", output: null, error: { code: "engine.docker", message: "token=abc123", httpStatus: 409, issues: null } })], meta));
    expect(json.results[0]).toMatchObject({ name: "web", environment: "Este equipo", status: "failed", stdout: null });
    expect(JSON.stringify(json)).not.toContain("S3creta");
  });
});
