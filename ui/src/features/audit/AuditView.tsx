import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, History, PencilLine, Play, Search, ShieldCheck, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApi, useServerId, useServerView } from "@/app/server";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { api } from "@/shared/ipc/client";
import type { AuditFix } from "@/shared/ipc/bindings/AuditFix";
import type { AuditReport } from "@/shared/ipc/bindings/AuditReport";
import type { AuditTargetKind } from "@/shared/ipc/bindings/AuditTargetKind";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { RiskLevelBadge } from "@/shared/ui/HostRisks";
import {
  fileName,
  findingFix,
  findingText,
  findingWhy,
  groupByTarget,
  LEVELS,
  matchesFinding,
  reasonText,
  shortDate,
  toHtml,
  toJson,
  toMarkdown,
  vulnStatusText,
  type KindFilter,
  type LevelFilter,
  type TargetGroup,
} from "./model";
import { useAudits } from "./store";

const KINDS: AuditTargetKind[] = ["container", "service", "image"];

/** Los motivos se escriben para ir tras «No analizado:»; solos, con mayúscula. */
function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export function AuditView() {
  return (
    <RequireConnection>
      <Audit />
    </RequireConnection>
  );
}

function Audit() {
  const { t, i18n } = useTranslation();
  const server = useServerId();
  const view = useServerView();
  const remote = !!view?.remote;
  const bound = useApi();
  const qc = useQueryClient();
  const run = useAudits((s) => s.runs[server]);
  const start = useAudits((s) => s.start);
  const cancel = useAudits((s) => s.cancel);
  const containers = useQuery({ queryKey: keys.containers, queryFn: bound.containersList });
  const tools = useQuery({ queryKey: ["auditTools"], queryFn: api.auditTools, enabled: !remote, staleTime: 60_000 });
  const history = useQuery({ queryKey: ["auditHistory"], queryFn: bound.auditHistory });

  const [container, setContainer] = useState("");
  const [compose, setCompose] = useState(true);
  const [images, setImages] = useState(true);
  const [vulns, setVulns] = useState(false);
  const running = run?.status === "running";
  const readyTool = (tools.data ?? []).find((x) => x.dbReady);

  const submit = () =>
    void start(server, { container: container || null, includeCompose: compose, includeImages: images, scanVulnerabilities: vulns && !remote, tool: null }, (ok) => {
      // Solo una auditoría terminada se guarda en el historial.
      if (ok) void qc.invalidateQueries({ queryKey: ["auditHistory"] });
    });

  const p = run?.progress;
  return (
    <div className="px-8 py-6">
      <p className="mb-4 flex max-w-[820px] items-start gap-2 text-[12.5px] text-ink-2">
        <ShieldCheck size={16} className="mt-px shrink-0 text-beacon-strong" aria-hidden />
        {t("audit.intro")}
      </p>

      <section aria-labelledby="audit-options" className="mb-5 rounded-xl border border-line bg-surface p-4">
        <h2 id="audit-options" className="mb-3 text-[13.5px] font-semibold">
          {t("audit.options")}
        </h2>
        <div className="grid gap-4 md:grid-cols-[minmax(0,320px)_1fr]">
          <label className="block text-[12.5px] font-medium text-ink-2">
            {t("audit.scope")}
            <select
              className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] text-ink"
              value={container}
              onChange={(e) => setContainer(e.target.value)}
              disabled={running}
            >
              <option value="">{t("audit.scopeAll")}</option>
              {(containers.data ?? []).map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          <div className="space-y-2">
            <Checkbox checked={compose} onChange={setCompose} disabled={running || !!container} label={t("audit.includeCompose")} />
            <Checkbox checked={images} onChange={setImages} disabled={running || !!container} label={t("audit.includeImages")} />
            <Checkbox
              checked={vulns && !remote}
              onChange={setVulns}
              disabled={running || remote}
              label={t("audit.scanVulns")}
              hint={capitalize(
                remote
                  ? t("audit.vuln.remote")
                  : tools.isPending
                    ? t("common.loading")
                    : readyTool
                      ? t("audit.vuln.tool", { name: readyTool.name, version: readyTool.version ?? "?", date: readyTool.dbUpdated ? shortDate(readyTool.dbUpdated, i18n.language) : t("audit.vuln.unknownDate") })
                      : (tools.data ?? []).length > 0
                        ? t("audit.vuln.noDatabase")
                        : t("audit.vuln.noTool"),
              )}
            />
          </div>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-2">
          {running ? (
            <Button onClick={() => cancel(server)}>
              <Square size={13} />
              {t("audit.cancel")}
            </Button>
          ) : (
            <Button variant="primary" onClick={submit}>
              <Play size={14} />
              {t("audit.run")}
            </Button>
          )}
          {running && (
            <span className="flex items-center gap-2 text-[12.5px] text-ink-2" aria-live="polite">
              <Spinner size={14} />
              {p ? t(`audit.progress.${p.step}`, { done: p.done + 1, total: p.total, current: p.current ?? "" }) : t("audit.running")}
            </span>
          )}
          {run?.status === "cancelled" && <span className="text-[12.5px] text-ink-3">{t("audit.cancelled")}</span>}
        </div>
      </section>

      {run?.status === "failed" && run.error && (
        <div className="mb-4">
          <Banner tone="error">{t("audit.failed", { message: run.error.message })}</Banner>
        </div>
      )}

      {run?.report ? <Report report={run.report} /> : <p className="mb-6 text-[13px] text-ink-3">{t("audit.noReport")}</p>}

      <HistoryList loading={history.isPending} items={history.data ?? []} current={run?.report?.id ?? null} />
    </div>
  );
}

function Report({ report }: { report: AuditReport }) {
  const { t, i18n } = useTranslation();
  const tt = t as unknown as (k: string, o?: Record<string, unknown>) => string;
  const server = useServerId();
  const view = useServerView();
  const [level, setLevel] = useState<LevelFilter>("all");
  const [kind, setKind] = useState<KindFilter>("all");
  const [query, setQuery] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const visible = useMemo(() => report.findings.filter((f) => matchesFinding(f, level, kind, query, tt)), [report, level, kind, query, tt]);
  const groups = useMemo(() => groupByTarget(visible), [visible]);
  const serverName = view?.name || t("servers.local");
  const clean = report.targets.filter((x) => !report.findings.some((f) => f.target.kind === x.target.kind && f.target.id === x.target.id));

  const doExport = async (format: "json" | "markdown" | "html") => {
    setSaved(null);
    setExportError(null);
    const content =
      format === "json" ? toJson(report) : format === "markdown" ? toMarkdown(tt, report, serverName, i18n.language) : toHtml(tt, report, serverName, i18n.language);
    try {
      const path = await api.auditExportSave(format, content, fileName(report, server));
      if (path) setSaved(path);
    } catch (e) {
      setExportError((e as { message: string }).message);
    }
  };

  return (
    <section aria-labelledby="audit-result" className="mb-8">
      <div className="mb-3 flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <h2 id="audit-result" className="text-[15px] font-semibold">
          {t("audit.summary.title")}
        </h2>
        <span className="text-[12.5px] text-ink-3">
          {t("audit.summary.meta", {
            date: `${shortDate(report.startedAt, i18n.language)} ${new Date(report.startedAt).toLocaleTimeString(i18n.language)}`,
            count: report.targets.length,
            docker: report.dockerVersion ?? "?",
          })}
        </span>
        <div className="ml-auto flex flex-wrap gap-1.5">
          {(["json", "markdown", "html"] as const).map((f) => (
            <Button key={f} size="sm" onClick={() => void doExport(f)}>
              <Download size={13} />
              {t(`audit.export.${f}`)}
            </Button>
          ))}
        </div>
      </div>
      {saved && (
        <div className="mb-3">
          <Banner tone="info">{t("audit.export.saved", { path: saved })}</Banner>
        </div>
      )}
      {exportError && (
        <div className="mb-3">
          <Banner tone="error">{t("audit.export.error", { message: exportError })}</Banner>
        </div>
      )}

      <ul className="mb-4 flex flex-wrap gap-2" aria-label={t("audit.summary.counts")}>
        {LEVELS.map((l) => (
          <li key={l} className="flex items-center gap-2 rounded-lg border border-line bg-surface px-3 py-1.5 text-[13px]">
            <RiskLevelBadge level={l} />
            <span className="font-semibold tabular-nums">{report.counts[l] ?? 0}</span>
          </li>
        ))}
      </ul>

      <Daemon report={report} />

      <div className="mb-3 flex flex-wrap items-center gap-3">
        <Segmented<LevelFilter>
          label={t("audit.filter.level")}
          value={level}
          onChange={setLevel}
          options={(["all", ...LEVELS] as LevelFilter[]).map((l) => ({ value: l, label: l === "all" ? t("audit.filter.all") : t(`hostRisk.level.${l}`) }))}
        />
        <label className="text-[12.5px] text-ink-2">
          <span className="sr-only">{t("audit.filter.kind")}</span>
          <select className="h-8 rounded-md border border-line bg-surface px-2 text-[13px] text-ink" value={kind} onChange={(e) => setKind(e.target.value as KindFilter)} aria-label={t("audit.filter.kind")}>
            <option value="all">{t("audit.filter.allKinds")}</option>
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {t(`audit.kindPlural.${k}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="relative max-w-[320px] min-w-[200px] flex-1">
          <span className="sr-only">{t("audit.filter.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("audit.filter.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("audit.filter.count", { count: visible.length })}</span>
      </div>

      {report.findings.length === 0 ? (
        <Banner tone="info">{t("audit.summary.none")}</Banner>
      ) : groups.length === 0 ? (
        <p className="py-6 text-center text-[13px] text-ink-2">{t("audit.filter.noMatches")}</p>
      ) : (
        <ul className="space-y-3">
          {groups.map((g) => (
            <Group key={g.key} g={g} />
          ))}
        </ul>
      )}

      {clean.length > 0 && level === "all" && kind === "all" && !query && (
        <p className="mt-3 text-[12.5px] text-ink-3">{t("audit.clean", { count: clean.length, names: clean.map((c) => c.target.name).join(", ") })}</p>
      )}

      <Vulns report={report} />

      {report.skipped.length > 0 && (
        <div className="mt-5">
          <h3 className="mb-2 text-[13.5px] font-semibold">{t("audit.skipped.title")}</h3>
          <ul className="space-y-1 text-[12.5px] text-ink-2">
            {report.skipped.map((s, i) => (
              <li key={i}>
                {s.target.project ? `${s.target.project}: ` : ""}
                {reasonText(tt, s.reason)}
                {s.detail && <span className="selectable block font-mono text-[11.5px] text-ink-3">{s.detail}</span>}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function Daemon({ report }: { report: AuditReport }) {
  const { t } = useTranslation();
  const d = report.daemon;
  const items: Array<[string, boolean, string?]> = [
    ["rootless", d.rootless],
    ["userns", d.userns],
    ["seccomp", d.seccomp, d.seccompProfile ?? undefined],
    ["apparmor", d.apparmor],
    ["selinux", d.selinux],
  ];
  return (
    <p className="mb-4 text-[12.5px] text-ink-2">
      <span className="font-medium">{t("audit.daemon.title")}:</span>{" "}
      {items.map(([k, v, extra]) => `${t(`audit.daemon.${k}`)} ${v ? t("audit.daemon.yes") : t("audit.daemon.no")}${extra ? ` (${extra})` : ""}`).join(" · ")}
      {` · ${t("audit.daemon.cgroup")} ${d.cgroupVersion ?? "?"}`}
    </p>
  );
}

function Group({ g }: { g: TargetGroup }) {
  const { t } = useTranslation();
  const tt = t as unknown as (k: string, o?: Record<string, unknown>) => string;
  const fixes: AuditFix[] = g.kind === "container" ? g.findings.flatMap((f) => (f.fix ? [f.fix] : [])) : [];
  const propose = () => {
    useContainersNav.getState().showEdit(g.id, g.name, fixes);
    useApp.getState().navigate("containers");
  };
  return (
    <li className="rounded-xl border border-line bg-surface">
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2.5">
        <RiskLevelBadge level={g.worst} />
        <span className="text-[11.5px] text-ink-3 uppercase">{t(`audit.kind.${g.kind}`)}</span>
        <h3 className="selectable min-w-0 truncate text-[13.5px] font-semibold">{g.name}</h3>
        {g.project && <span className="text-[12px] text-ink-3">{t("audit.project", { name: g.project })}</span>}
        {fixes.length > 0 && (
          <Button size="sm" className="ml-auto" onClick={propose} title={t("audit.proposeHint")}>
            <PencilLine size={13} />
            {t("audit.propose", { count: fixes.length })}
          </Button>
        )}
      </div>
      <ul className="divide-y divide-line">
        {g.findings.map((f) => {
          const why = findingWhy(tt, f);
          const fix = findingFix(tt, f);
          return (
            <li key={f.key} className="grid grid-cols-[76px_minmax(0,1fr)] gap-x-2 px-4 py-2.5">
              <div className="pt-px">
                <RiskLevelBadge level={f.level} />
              </div>
              <p className="selectable text-[13px]">{findingText(tt, f)}</p>
              {(why || fix) && (
                <dl className="col-start-2 mt-1 space-y-0.5 text-[12.5px] text-ink-2">
                  {why && (
                    <div>
                      <dt className="inline font-medium">{t("audit.finding.why")}: </dt>
                      <dd className="inline">{why}</dd>
                    </div>
                  )}
                  {fix && (
                    <div>
                      <dt className="inline font-medium">{t("audit.finding.fix")}: </dt>
                      <dd className="inline">{fix}</dd>
                    </div>
                  )}
                </dl>
              )}
            </li>
          );
        })}
      </ul>
    </li>
  );
}

function Vulns({ report }: { report: AuditReport }) {
  const { t, i18n } = useTranslation();
  const tt = t as unknown as (k: string, o?: Record<string, unknown>) => string;
  const v = report.vulnerabilities;
  return (
    <div className="mt-6">
      <h3 className="mb-2 text-[13.5px] font-semibold">{t("audit.vuln.title")}</h3>
      {v.tools.map((tool) => (
        <p key={tool.name} className="text-[12.5px] text-ink-2">
          {tool.dbReady
            ? t("audit.vuln.tool", { name: tool.name, version: tool.version ?? "?", date: tool.dbUpdated ? shortDate(tool.dbUpdated, i18n.language) : t("audit.vuln.unknownDate") })
            : t("audit.vuln.toolNotReady", { name: tool.name, version: tool.version ?? "?", reason: reasonText(tt, tool.problem ?? "audit.vuln.noDatabase") })}
        </p>
      ))}
      {v.notAnalyzed && (
        <Banner tone={v.requested ? "warn" : "info"}>{t("audit.vuln.notAnalyzed", { reason: reasonText(tt, v.notAnalyzed) })}</Banner>
      )}
      {v.images.length > 0 && (
        <ul className="mt-2 space-y-2">
          {v.images.map((i) => (
            <li key={i.image} className="rounded-lg border border-line bg-surface px-3 py-2 text-[12.5px]">
              <p className="selectable font-mono text-[12.5px]">{i.image}</p>
              <p className={i.status === "done" ? "text-ink-2" : "text-warn"}>{vulnStatusText(tt, i, i18n.language)}</p>
              {i.status === "done" && i.top.length > 0 && (
                <details className="mt-1">
                  <summary className="cursor-pointer text-ink-3">{t("audit.vuln.list", { count: i.top.length })}</summary>
                  <table className="mt-1 w-full text-left text-[12px]">
                    <thead className="text-ink-3">
                      <tr>
                        <th className="py-0.5 pr-2 font-medium">{t("audit.vuln.id")}</th>
                        <th className="py-0.5 pr-2 font-medium">{t("audit.vuln.package")}</th>
                        <th className="py-0.5 pr-2 font-medium">{t("audit.vuln.severity")}</th>
                        <th className="py-0.5 font-medium">{t("audit.vuln.fixed")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {i.top.map((x) => (
                        <tr key={`${x.id}-${x.package}-${x.installed}`}>
                          <td className="selectable py-0.5 pr-2 font-mono">{x.id}</td>
                          <td className="selectable py-0.5 pr-2">
                            {x.package} {x.installed}
                          </td>
                          <td className="py-0.5 pr-2">{x.severity}</td>
                          <td className="selectable py-0.5">{x.fixed ?? t("audit.vuln.noFix")}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </details>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function HistoryList({ loading, items, current }: { loading: boolean; items: Array<{ id: string; startedAt: string; container: string | null; targets: number; counts: { [k in string]?: number } }>; current: string | null }) {
  const { t, i18n } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const show = useAudits((s) => s.show);
  const [error, setError] = useState<string | null>(null);
  const open = async (id: string) => {
    setError(null);
    try {
      show(server, await bound.auditGet(id));
    } catch (e) {
      setError((e as { message: string }).message);
    }
  };
  return (
    <section aria-labelledby="audit-history">
      <h2 id="audit-history" className="mb-2 flex items-center gap-2 text-[13.5px] font-semibold">
        <History size={15} aria-hidden />
        {t("audit.history.title")}
      </h2>
      {error && <Banner tone="error">{error}</Banner>}
      {loading ? (
        <Spinner />
      ) : items.length === 0 ? (
        <p className="text-[12.5px] text-ink-3">{t("audit.history.empty")}</p>
      ) : (
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface text-[12.5px]">
          {items.map((h) => (
            <li key={h.id} className="flex flex-wrap items-center gap-3 px-4 py-2">
              <span className="tabular-nums">{`${shortDate(h.startedAt, i18n.language)} ${new Date(h.startedAt).toLocaleTimeString(i18n.language)}`}</span>
              <span className="text-ink-3">{h.container ? t("audit.history.one", { name: h.container }) : t("audit.history.all", { count: h.targets })}</span>
              <span className="flex gap-1.5">
                {LEVELS.map((l) => (
                  <span key={l} className="text-ink-2" title={t(`hostRisk.level.${l}`)}>
                    {t(`hostRisk.level.${l}`)} {h.counts[l] ?? 0}
                  </span>
                ))}
              </span>
              {h.id === current ? (
                <span className="ml-auto text-ink-3">{t("audit.history.shown")}</span>
              ) : (
                <Button size="sm" className="ml-auto" onClick={() => void open(h.id)}>
                  {t("audit.history.open")}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
