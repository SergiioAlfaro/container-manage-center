// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Comandos masivos (1.11.0): el formulario del comando, los ejemplos, los
 * comandos guardados y la exportación de resultados. Lógica pura: la
 * validación definitiva, los argumentos exactos y el riesgo los da el backend
 * (`bulk_exec_preview`).
 */
import type { BulkCommand } from "@/shared/ipc/bindings/BulkCommand";
import type { ExecItemResult } from "@/shared/ipc/bindings/ExecItemResult";
import type { ExecItemStatus } from "@/shared/ipc/bindings/ExecItemStatus";
import type { ExecTemplate } from "@/shared/ipc/bindings/ExecTemplate";
import { redactText } from "@/shared/secrets";
import type { BulkRow } from "./model";

export const INTERPRETERS = ["/bin/sh", "/bin/bash", "/bin/ash", "/usr/bin/python3"] as const;
export const CUSTOM = "custom";
export const MAX_TIMEOUT_SECS = 3600;
export const DEFAULT_TIMEOUT_SECS = 30;

export interface ExecForm {
  mode: "direct" | "script";
  program: string;
  /** Un argumento por línea: los espacios forman parte del argumento. */
  argsText: string;
  /** Uno de `INTERPRETERS` o `CUSTOM`. */
  interpreter: string;
  customInterpreter: string;
  flag: string;
  script: string;
  user: string;
  workdir: string;
  timeoutSecs: number;
}

export const EMPTY_FORM: ExecForm = {
  mode: "direct",
  program: "",
  argsText: "",
  interpreter: "/bin/sh",
  customInterpreter: "",
  flag: "-c",
  script: "",
  user: "",
  workdir: "",
  timeoutSecs: DEFAULT_TIMEOUT_SECS,
};

/** Argumentos del cuadro de texto: uno por línea, tal cual. Una línea vacía
 *  final (al pulsar Intro) no cuenta. */
export function argsOf(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  return lines;
}

export function interpreterOf(f: ExecForm): string {
  return f.interpreter === CUSTOM ? f.customInterpreter.trim() : f.interpreter;
}

export function commandOf(f: ExecForm): BulkCommand {
  return {
    command:
      f.mode === "direct"
        ? { mode: "direct", program: f.program.trim(), args: argsOf(f.argsText) }
        : { mode: "script", interpreter: interpreterOf(f), flag: f.flag.trim(), script: f.script },
    user: f.user.trim() || null,
    workdir: f.workdir.trim() || null,
    timeoutSecs: f.timeoutSecs,
  };
}

/** El formulario está completo para pedir la vista previa. */
export function formReady(f: ExecForm): boolean {
  const okTime = Number.isInteger(f.timeoutSecs) && f.timeoutSecs >= 1 && f.timeoutSecs <= MAX_TIMEOUT_SECS;
  return okTime && (f.mode === "direct" ? f.program.trim() !== "" : interpreterOf(f) !== "" && f.script.trim() !== "");
}

// ------------------------------------------------------------- plantillas

/** Ejemplos incluidos (no se guardan; se pueden copiar a «guardados»). No
 *  garantizan que funcionen en cualquier imagen: dependen de lo que tenga. */
export const EXAMPLES: Array<{ key: string; template: Omit<ExecTemplate, "id" | "name" | "description"> }> = [
  { key: "osRelease", template: tpl("direct", { program: "cat", args: ["/etc/os-release"] }) },
  { key: "disk", template: tpl("direct", { program: "df", args: ["-h"] }) },
  { key: "memory", template: tpl("direct", { program: "cat", args: ["/proc/meminfo"] }) },
  { key: "processes", template: tpl("direct", { program: "ps", args: [] }) },
  {
    key: "packagesIndex",
    template: tpl("script", {
      interpreter: "/bin/sh",
      flag: "-c",
      script:
        "if command -v apt-get >/dev/null 2>&1; then apt-get update\nelif command -v apk >/dev/null 2>&1; then apk update\nelif command -v dnf >/dev/null 2>&1; then dnf makecache\nelse echo 'No hay un gestor de paquetes conocido' >&2; exit 1\nfi",
      user: "root",
      timeoutSecs: 300,
    }),
  },
  {
    key: "network",
    template: tpl("script", {
      interpreter: "/bin/sh",
      flag: "-c",
      script: "cat /etc/hosts\necho\ncat /etc/resolv.conf\necho\nip addr 2>/dev/null || cat /proc/net/dev",
    }),
  },
  { key: "environment", template: tpl("direct", { program: "env", args: [] }) },
];

function tpl(mode: "direct" | "script", over: Partial<ExecTemplate>): Omit<ExecTemplate, "id" | "name" | "description"> {
  return { mode, program: "", args: [], interpreter: "", flag: "", script: "", user: "", workdir: "", timeoutSecs: DEFAULT_TIMEOUT_SECS, ...over };
}

export function formFromTemplate(t: Omit<ExecTemplate, "id" | "name" | "description">): ExecForm {
  const known = (INTERPRETERS as readonly string[]).includes(t.interpreter);
  return {
    mode: t.mode,
    program: t.program,
    argsText: t.args.join("\n"),
    interpreter: t.mode === "script" ? (known ? t.interpreter : CUSTOM) : EMPTY_FORM.interpreter,
    customInterpreter: t.mode === "script" && !known ? t.interpreter : "",
    flag: t.mode === "script" ? t.flag : EMPTY_FORM.flag,
    script: t.script,
    user: t.user,
    workdir: t.workdir,
    timeoutSecs: t.timeoutSecs,
  };
}

export function templateFromForm(f: ExecForm, id: string, name: string, description: string): ExecTemplate {
  const direct = f.mode === "direct";
  return {
    id,
    name: name.trim(),
    description: description.trim(),
    mode: f.mode,
    program: direct ? f.program.trim() : "",
    args: direct ? argsOf(f.argsText) : [],
    interpreter: direct ? "" : interpreterOf(f),
    flag: direct ? "" : f.flag.trim(),
    script: direct ? "" : f.script,
    user: f.user.trim(),
    workdir: f.workdir.trim(),
    timeoutSecs: f.timeoutSecs,
  };
}

export function newTemplateId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `t-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

// ------------------------------------------------------------- destinos

/** Solo se ejecuta en contenedores en ejecución; los demás se informan. */
export function splitTargets(rows: BulkRow[]): { run: BulkRow[]; skip: BulkRow[] } {
  return { run: rows.filter((r) => r.c.state === "running"), skip: rows.filter((r) => r.c.state !== "running") };
}

// ------------------------------------------------------------- resultados

/** Estado que se muestra: los de la petición más «pendiente» y «en ejecución». */
export type ShownStatus = ExecItemStatus | "pending" | "running";

export function shownStatus(r: ExecItemResult | undefined, running: boolean): ShownStatus {
  return r ? r.status : running ? "running" : "pending";
}

export interface ExportMeta {
  /** Fila de la tabla (imagen y servidor) de cada destino, por índice. */
  rows: Array<Pick<BulkRow, "envName"> & { image: string }>;
  command: string;
  user: string | null;
  workdir: string | null;
  timeoutSecs: number;
  batch: string | null;
  startedAt: string | null;
  durationMs: number | null;
}

type T = (k: string, o?: Record<string, unknown>) => string;

/** Informe en Markdown con la salida de cada contenedor. Los secretos se
 *  ocultan aquí y otra vez en el backend al guardarlo. */
export function exportMarkdown(t: T, results: ExecItemResult[], meta: ExportMeta): string {
  const out: string[] = [];
  out.push(`# ${t("bulk.exec.export.title")}`, "");
  out.push(`- ${t("bulk.exec.export.command")}: \`${meta.command}\``);
  if (meta.user) out.push(`- ${t("bulk.exec.user")}: ${meta.user}`);
  if (meta.workdir) out.push(`- ${t("bulk.exec.workdir")}: ${meta.workdir}`);
  out.push(`- ${t("bulk.exec.timeout")}: ${meta.timeoutSecs} s`);
  if (meta.batch) out.push(`- ${t("activity.batch")}: ${meta.batch}`);
  if (meta.startedAt) out.push(`- ${t("bulk.exec.export.started")}: ${meta.startedAt}`);
  if (meta.durationMs !== null) out.push(`- ${t("bulk.exec.export.duration")}: ${meta.durationMs} ms`);
  out.push("");
  for (const r of results) {
    const m = meta.rows[r.index];
    out.push(`## ${r.target.name}`, "");
    out.push(`- ID: ${r.target.id}`);
    if (m) out.push(`- ${t("bulk.col.image")}: ${m.image}`, `- ${t("bulk.col.env")}: ${m.envName}`);
    out.push(`- ${t("bulk.exec.export.status")}: ${t(`bulk.exec.status.${r.status}`)}`);
    if (r.output?.exitCode !== null && r.output?.exitCode !== undefined) out.push(`- ${t("bulk.exec.exitCode")}: ${r.output.exitCode}`);
    if (r.output?.startedAt) out.push(`- ${t("bulk.exec.export.started")}: ${r.output.startedAt}`);
    out.push(`- ${t("bulk.exec.export.duration")}: ${r.durationMs} ms`);
    if (r.error) out.push(`- ${t("bulk.exec.error")}: ${r.error.message}`);
    if (r.output) {
      out.push("", `### stdout${r.output.stdoutTruncated ? ` (${t("bulk.exec.truncated")})` : ""}`, "", "```", r.output.stdout.replace(/```/g, "ʼʼʼ"), "```");
      out.push("", `### stderr${r.output.stderrTruncated ? ` (${t("bulk.exec.truncated")})` : ""}`, "", "```", r.output.stderr.replace(/```/g, "ʼʼʼ"), "```");
    }
    out.push("");
  }
  return redactText(out.join("\n"));
}

/** Los mismos datos en JSON, con los secretos ocultos. */
export function exportJson(results: ExecItemResult[], meta: ExportMeta): string {
  const data = {
    command: meta.command,
    user: meta.user,
    workdir: meta.workdir,
    timeoutSecs: meta.timeoutSecs,
    batch: meta.batch,
    startedAt: meta.startedAt,
    durationMs: meta.durationMs,
    results: results.map((r) => ({
      name: r.target.name,
      id: r.target.id,
      server: r.target.server,
      image: meta.rows[r.index]?.image ?? null,
      environment: meta.rows[r.index]?.envName ?? null,
      status: r.status,
      exitCode: r.output?.exitCode ?? null,
      startedAt: r.output?.startedAt ?? null,
      durationMs: r.durationMs,
      stdout: r.output?.stdout ?? null,
      stderr: r.output?.stderr ?? null,
      stdoutTruncated: r.output?.stdoutTruncated ?? false,
      stderrTruncated: r.output?.stderrTruncated ?? false,
      error: r.error?.message ?? null,
    })),
  };
  return redactText(JSON.stringify(data, null, 2));
}
