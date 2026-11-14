import type { AuditFinding } from "@/shared/ipc/bindings/AuditFinding";
import type { AuditReport } from "@/shared/ipc/bindings/AuditReport";
import type { AuditTargetKind } from "@/shared/ipc/bindings/AuditTargetKind";
import type { ImageVulns } from "@/shared/ipc/bindings/ImageVulns";
import type { LinkTarget } from "@/shared/ipc/bindings/LinkTarget";
import type { RiskLevel } from "@/shared/ipc/bindings/RiskLevel";
import { redactText } from "@/shared/secrets";

export const LEVELS: RiskLevel[] = ["critical", "high", "medium", "low"];
export type LevelFilter = "all" | RiskLevel;
export type KindFilter = "all" | AuditTargetKind;

type T = (k: string, o?: Record<string, unknown>) => string;

/** Nombre corto de la comprobación (`risk.runtimeSocket` → `runtimeSocket`). */
export function checkName(code: string): string {
  return code.split(".").pop() ?? code;
}

/** Qué ocurre (la explicación con sus datos). */
export function findingText(t: T, f: AuditFinding): string {
  const key = f.code.startsWith("risk.") ? `issues.${f.code}` : `audit.check.${checkName(f.code)}`;
  return t(key, { ...f.params, defaultValue: f.code });
}

/** Por qué importa y qué hacer. */
export function findingWhy(t: T, f: AuditFinding): string {
  return t(`audit.why.${checkName(f.code)}`, { ...f.params, defaultValue: "" });
}
export function findingFix(t: T, f: AuditFinding): string {
  return t(`audit.fix.${checkName(f.code)}`, { ...f.params, defaultValue: "" });
}

export function matchesFinding(f: AuditFinding, level: LevelFilter, kind: KindFilter, query: string, t: T): boolean {
  if (level !== "all" && f.level !== level) return false;
  if (kind !== "all" && f.target.kind !== kind) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [f.target.name, f.target.project ?? "", f.code, findingText(t, f)].some((x) => x.toLowerCase().includes(q));
}

export interface TargetGroup {
  key: string;
  kind: AuditTargetKind;
  name: string;
  project: string | null;
  id: string;
  worst: RiskLevel;
  findings: AuditFinding[];
}

const rank = (l: RiskLevel) => LEVELS.indexOf(l);

/** E6: contenedores auditados (los servicios e imágenes no se monitorizan ni
 *  se copian como tales). */
export function containerTargets(report: AuditReport): LinkTarget[] {
  const seen = new Set<string>();
  return report.targets
    .filter((x) => x.target.kind === "container" && !seen.has(x.target.id) && seen.add(x.target.id))
    .map((x) => ({ id: x.target.id, name: x.target.name, project: x.target.project }));
}

/** E6: el peor nivel de los hallazgos de cada contenedor, por id. */
export function worstByTarget(report: AuditReport): Map<string, RiskLevel> {
  const m = new Map<string, RiskLevel>();
  for (const f of report.findings) {
    if (f.target.kind !== "container") continue;
    const cur = m.get(f.target.id);
    if (cur === undefined || rank(f.level) < rank(cur)) m.set(f.target.id, f.level);
  }
  return m;
}

/** Hallazgos agrupados por contenedor, servicio o imagen, los peores primero. */
export function groupByTarget(findings: AuditFinding[]): TargetGroup[] {
  const m = new Map<string, TargetGroup>();
  for (const f of findings) {
    const key = `${f.target.kind}:${f.target.id}`;
    let g = m.get(key);
    if (!g) {
      g = { key, kind: f.target.kind, name: f.target.name, project: f.target.project, id: f.target.id, worst: f.level, findings: [] };
      m.set(key, g);
    }
    g.findings.push(f);
    if (rank(f.level) < rank(g.worst)) g.worst = f.level;
  }
  const groups = [...m.values()];
  for (const g of groups) g.findings.sort((a, b) => rank(a.level) - rank(b.level));
  return groups.sort((a, b) => rank(a.worst) - rank(b.worst) || b.findings.length - a.findings.length || a.name.localeCompare(b.name));
}

/** Texto del estado de las vulnerabilidades de una imagen. Nunca «0» si no
 *  terminó el análisis. */
export function vulnStatusText(t: T, v: ImageVulns, lang: string): string {
  switch (v.status) {
    case "done": {
      const date = v.dbUpdated ? shortDate(v.dbUpdated, lang) : t("audit.vuln.unknownDate");
      return v.counts.critical + v.counts.high + v.counts.medium + v.counts.low + v.counts.unknown === 0
        ? t("audit.vuln.noneKnown", { tool: v.tool, date })
        : t("audit.vuln.found", { ...v.counts, tool: v.tool, date });
    }
    case "notAnalyzed":
      return t("audit.vuln.notAnalyzedImage", { reason: reasonText(t, v.reason) });
    case "failed":
      return t("audit.vuln.failed", { tool: v.tool, message: v.message });
  }
}

/** `audit.vuln.toolError:mensaje` → texto con el mensaje. */
export function reasonText(t: T, reason: string): string {
  const i = reason.indexOf(":");
  if (i > 0 && reason.startsWith("audit.")) {
    return t(reason.slice(0, i), { message: reason.slice(i + 1), defaultValue: reason });
  }
  return t(reason, { defaultValue: reason });
}

export function shortDate(iso: string, lang: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString(lang, { year: "numeric", month: "short", day: "numeric" });
}

export function fileName(r: AuditReport, server: string): string {
  return `auditoria-${server}-${r.id}`;
}

// ------------------------------------------------------------ exportación
// El contenido se genera aquí, en el idioma de la interfaz; Rust lo vuelve a
// pasar por la ocultación de secretos y lo guarda con permisos 0600.

export function toJson(r: AuditReport): string {
  return redactText(JSON.stringify(r, null, 2));
}

function mdEscape(s: string): string {
  return s.replace(/([\\`*_[\]#|<>])/g, "\\$1");
}

function header(t: T, r: AuditReport, serverName: string, lang: string) {
  return {
    title: t("audit.export.title", { server: serverName }),
    date: `${shortDate(r.startedAt, lang)} ${new Date(r.startedAt).toLocaleTimeString(lang)}`,
    docker: r.dockerVersion ?? "?",
    scope: r.request.container ? t("audit.export.scopeOne", { name: r.request.container }) : t("audit.export.scopeAll"),
  };
}

function daemonLines(t: T, r: AuditReport): string[] {
  const d = r.daemon;
  const yes = (b: boolean) => (b ? t("audit.daemon.yes") : t("audit.daemon.no"));
  return [
    `${t("audit.daemon.rootless")}: ${yes(d.rootless)}`,
    `${t("audit.daemon.userns")}: ${yes(d.userns)}`,
    `${t("audit.daemon.seccomp")}: ${yes(d.seccomp)}${d.seccompProfile ? ` (${d.seccompProfile})` : ""}`,
    `${t("audit.daemon.apparmor")}: ${yes(d.apparmor)}`,
    `${t("audit.daemon.selinux")}: ${yes(d.selinux)}`,
    `${t("audit.daemon.cgroup")}: ${d.cgroupVersion ?? "?"}`,
  ];
}

function vulnLines(t: T, r: AuditReport, lang: string): string[] {
  const v = r.vulnerabilities;
  const out: string[] = [];
  for (const tool of v.tools) {
    out.push(
      tool.dbReady
        ? t("audit.vuln.tool", { name: tool.name, version: tool.version ?? "?", date: tool.dbUpdated ? shortDate(tool.dbUpdated, lang) : t("audit.vuln.unknownDate") })
        : t("audit.vuln.toolNotReady", { name: tool.name, version: tool.version ?? "?", reason: reasonText(t, tool.problem ?? "audit.vuln.noDatabase") }),
    );
  }
  if (v.notAnalyzed) out.push(t("audit.vuln.notAnalyzed", { reason: reasonText(t, v.notAnalyzed) }));
  for (const i of v.images) out.push(`${i.image}: ${vulnStatusText(t, i, lang)}`);
  return out;
}

export function toMarkdown(t: T, r: AuditReport, serverName: string, lang: string): string {
  const h = header(t, r, serverName, lang);
  const lines: string[] = [];
  lines.push(`# ${mdEscape(h.title)}`, "");
  lines.push(`- ${t("audit.export.date")}: ${h.date}`, `- Docker: ${h.docker}`, `- ${t("audit.export.scope")}: ${mdEscape(h.scope)}`, `- ${t("audit.export.readOnly")}`, "");
  lines.push(`## ${t("audit.summary.title")}`, "");
  lines.push(`| ${LEVELS.map((l) => t(`hostRisk.level.${l}`)).join(" | ")} |`, `|${LEVELS.map(() => "---").join("|")}|`, `| ${LEVELS.map((l) => r.counts[l] ?? 0).join(" | ")} |`, "");
  lines.push(`## ${t("audit.daemon.title")}`, "", ...daemonLines(t, r).map((l) => `- ${l}`), "");
  lines.push(`## ${t("audit.findings")}`, "");
  if (r.findings.length === 0) lines.push(t("audit.summary.none"), "");
  for (const g of groupByTarget(r.findings)) {
    lines.push(`### ${t(`audit.kind.${g.kind}`)}: ${mdEscape(g.name)}${g.project ? ` (${mdEscape(g.project)})` : ""}`, "");
    for (const f of g.findings) {
      lines.push(`- **${t(`hostRisk.level.${f.level}`)}** — ${mdEscape(findingText(t, f))}`);
      const why = findingWhy(t, f);
      const fix = findingFix(t, f);
      if (why) lines.push(`  - ${t("audit.finding.why")}: ${mdEscape(why)}`);
      if (fix) lines.push(`  - ${t("audit.finding.fix")}: ${mdEscape(fix)}`);
    }
    lines.push("");
  }
  lines.push(`## ${t("audit.vuln.title")}`, "", ...vulnLines(t, r, lang).map((l) => `- ${mdEscape(l)}`), "");
  if (r.skipped.length > 0) {
    lines.push(`## ${t("audit.skipped.title")}`, "");
    for (const s of r.skipped) lines.push(`- ${mdEscape(s.target.project ?? s.target.name)}: ${mdEscape(reasonText(t, s.reason))}${s.detail ? ` (${mdEscape(s.detail)})` : ""}`);
    lines.push("");
  }
  return redactText(lines.join("\n"));
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

const HTML_STYLE = `body{font:14px/1.5 system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;color:#1d1b26;background:#fff}
h1{color:#5b3fc4}h2{border-bottom:1px solid #ddd;padding-bottom:.2rem;margin-top:2rem}table{border-collapse:collapse}td,th{border:1px solid #ccc;padding:.3rem .6rem}
.l{display:inline-block;min-width:5.5rem;font-weight:600;font-size:12px;text-transform:uppercase}.critical{color:#b4232a}.high{color:#b45309}.medium{color:#8a6d00}.low{color:#555}
li{margin:.3rem 0}.sub{color:#555;font-size:13px}`;

export function toHtml(t: T, r: AuditReport, serverName: string, lang: string): string {
  const e = escapeHtml;
  const h = header(t, r, serverName, lang);
  const parts: string[] = [];
  parts.push(`<!doctype html><html lang="${e(lang)}"><head><meta charset="utf-8"><title>${e(h.title)}</title><style>${HTML_STYLE}</style></head><body>`);
  parts.push(`<h1>${e(h.title)}</h1><p>${e(t("audit.export.date"))}: ${e(h.date)} · Docker ${e(h.docker)} · ${e(h.scope)}</p><p class="sub">${e(t("audit.export.readOnly"))}</p>`);
  parts.push(`<h2>${e(t("audit.summary.title"))}</h2><table><tr>${LEVELS.map((l) => `<th class="${l}">${e(t(`hostRisk.level.${l}`))}</th>`).join("")}</tr><tr>${LEVELS.map((l) => `<td>${r.counts[l] ?? 0}</td>`).join("")}</tr></table>`);
  parts.push(`<h2>${e(t("audit.daemon.title"))}</h2><ul>${daemonLines(t, r).map((l) => `<li>${e(l)}</li>`).join("")}</ul>`);
  parts.push(`<h2>${e(t("audit.findings"))}</h2>`);
  if (r.findings.length === 0) parts.push(`<p>${e(t("audit.summary.none"))}</p>`);
  for (const g of groupByTarget(r.findings)) {
    parts.push(`<h3>${e(t(`audit.kind.${g.kind}`))}: ${e(g.name)}${g.project ? ` (${e(g.project)})` : ""}</h3><ul>`);
    for (const f of g.findings) {
      const why = findingWhy(t, f);
      const fix = findingFix(t, f);
      parts.push(
        `<li><span class="l ${f.level}">${e(t(`hostRisk.level.${f.level}`))}</span> ${e(findingText(t, f))}` +
          (why ? `<div class="sub">${e(t("audit.finding.why"))}: ${e(why)}</div>` : "") +
          (fix ? `<div class="sub">${e(t("audit.finding.fix"))}: ${e(fix)}</div>` : "") +
          `</li>`,
      );
    }
    parts.push(`</ul>`);
  }
  parts.push(`<h2>${e(t("audit.vuln.title"))}</h2><ul>${vulnLines(t, r, lang).map((l) => `<li>${e(l)}</li>`).join("")}</ul>`);
  if (r.skipped.length > 0) {
    parts.push(`<h2>${e(t("audit.skipped.title"))}</h2><ul>${r.skipped.map((s) => `<li>${e(s.target.project ?? s.target.name)}: ${e(reasonText(t, s.reason))}${s.detail ? ` (${e(s.detail)})` : ""}</li>`).join("")}</ul>`);
  }
  parts.push(`</body></html>`);
  return redactText(parts.join("\n"));
}
