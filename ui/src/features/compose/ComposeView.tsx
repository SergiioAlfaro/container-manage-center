import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { FilePlus2, FolderOpen, Play, RefreshCw, Search, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { composeLabel } from "@/features/settings/DockerSettings";
import { Badge } from "@/features/networks/badges";
import { api } from "@/shared/ipc/client";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { AddProjectDialog } from "./dialogs";
import { dirOf, hasFiles, matchesProject, refOf, simpleAction, type ProjectFilter } from "./model";
import { NewProject } from "./NewProject";
import { ProjectDetail } from "./ProjectDetail";
import { RunStatusLine } from "./RunPanel";
import { StateBadge } from "./StateBadge";
import { useComposeRuns, useRunCompose } from "./runs";
import { useComposeNav } from "./store";

function ProjectsTable() {
  const { t } = useTranslation();
  const nav = useComposeNav();
  const run = useRunCompose();
  const runs = useComposeRuns((s) => s.runs);
  const q = useQuery({ queryKey: keys.composeProjects, queryFn: api.composeProjects });
  const [filter, setFilter] = useState<ProjectFilter>("all");
  const [query, setQuery] = useState("");
  const [adding, setAdding] = useState(false);
  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((p) => matchesProject(p, filter, query)), [all, filter, query]);

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <Button variant="primary" onClick={() => setAdding(true)}>
          <FolderOpen size={15} />
          {t("composeList.add")}
        </Button>
        <Button onClick={nav.showNew}>
          <FilePlus2 size={15} />
          {t("composeList.new")}
        </Button>
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("composeList.refresh")}
        </Button>
      </div>
      <p className="mb-4 max-w-[780px] text-[12.5px] text-ink-3">{t("composeList.intro")}</p>

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<ProjectFilter>
          label={t("composeList.filterLabel")}
          value={filter}
          onChange={setFilter}
          options={(["all", "running", "stopped", "notDeployed"] as ProjectFilter[]).map((f) => ({ value: f, label: t(`composeList.filter.${f}`) }))}
        />
        <label className="relative max-w-[360px] min-w-[220px] flex-1">
          <span className="sr-only">{t("composeList.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("composeList.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("composeList.count", { count: visible.length })}</span>
      </div>

      {q.error && (
        <div className="mb-4">
          <Banner tone="error">{t("composeList.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : all.length === 0 && !q.error ? (
        <div className="py-20 text-center">
          <h2 className="text-[15px] font-semibold">{t("composeList.empty")}</h2>
          <p className="mx-auto mt-1.5 max-w-[460px] text-[13px] text-ink-2">{t("composeList.emptyBody")}</p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[820px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[24%] px-4 py-2 font-medium">{t("composeList.c.name")}</th>
                <th className="w-[16%] px-3 py-2 font-medium">{t("composeList.c.state")}</th>
                <th className="px-3 py-2 font-medium">{t("composeList.c.location")}</th>
                <th className="w-[20%] px-3 py-2 font-medium">{t("composeList.c.services")}</th>
                <th className="w-[84px] px-3 py-2 text-right font-medium">
                  <span className="sr-only">{t("composeList.c.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {visible.map((p) => {
                const r = runs[p.name];
                const busy = r?.status === "running";
                return (
                  <tr key={p.name} className="align-top">
                    <td className="px-4 py-2.5">
                      <button type="button" onClick={() => nav.showDetail(p.name)} className="block max-w-full truncate text-left font-mono text-[12.5px] font-medium hover:text-beacon-strong hover:underline">
                        {p.name}
                      </button>
                      <div className="mt-1 flex flex-wrap gap-1 empty:hidden">
                        {p.registered && <Badge tone="accent">{t("composeList.registered")}</Badge>}
                        {p.missingFiles.length > 0 && <Badge>{t("composeList.missing")}</Badge>}
                      </div>
                    </td>
                    <td className="px-3 py-2.5">
                      <StateBadge p={p} />
                      {r && (
                        <button type="button" className="mt-1 block text-left hover:underline" onClick={() => nav.showDetail(p.name, "output")}>
                          <RunStatusLine run={r} />
                        </button>
                      )}
                    </td>
                    <td className="selectable px-3 py-2.5 font-mono text-[12px] text-ink-2 [overflow-wrap:anywhere]">{p.workingDir ?? (p.files[0] ? dirOf(p.files[0]) : <span className="font-sans text-ink-3">{t("composeList.noLocation")}</span>)}</td>
                    <td className="px-3 py-2.5 font-mono text-[12px] text-ink-2 [overflow-wrap:anywhere]">{p.services.length > 0 ? p.services.join(", ") : <span className="font-sans text-ink-3">—</span>}</td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        {p.running > 0 ? (
                          <button type="button" aria-label={t("composeList.stop", { name: p.name })} title={t("composeList.stop", { name: p.name })} disabled={busy} onClick={() => void run(refOf(p), simpleAction("stop"))} className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink disabled:opacity-40">
                            <Square size={14} />
                          </button>
                        ) : p.total > 0 ? (
                          <button type="button" aria-label={t("composeList.start", { name: p.name })} title={t("composeList.start", { name: p.name })} disabled={busy} onClick={() => void run(refOf(p), simpleAction("start"))} className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink disabled:opacity-40">
                            <Play size={15} />
                          </button>
                        ) : (
                          <button type="button" aria-label={t("composeList.open", { name: p.name })} title={hasFiles(p) ? t("composeList.open", { name: p.name }) : undefined} onClick={() => nav.showDetail(p.name)} className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink">
                            <Play size={15} />
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {visible.length === 0 && <p className="px-4 py-10 text-center text-[13px] text-ink-2">{t("composeList.noMatches")}</p>}
        </div>
      )}
      {adding && <AddProjectDialog onClose={() => setAdding(false)} onAdded={(name) => nav.showDetail(name)} />}
    </div>
  );
}

/** Si Compose no está disponible, se explica cómo instalarlo (D-032). */
function RequireCompose({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const q = useQuery({ queryKey: keys.compose, queryFn: api.composeStatus, staleTime: 60_000 });
  if (q.isPending) {
    return (
      <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
        <Spinner /> {t("settingsDocker.composeChecking")}
      </div>
    );
  }
  if (q.data?.status === "available") return <>{children}</>;
  return (
    <div className="mx-auto max-w-[640px] px-8 py-16">
      <h2 className="text-[15px] font-semibold">{t("composeList.unavailable")}</h2>
      <p className="mt-1.5 text-[13px] text-ink-2">{q.error ? (q.error as { message: string }).message : composeLabel(q.isFetching, q.data, t)}</p>
      <p className="mt-4 text-[13px]">{t("settingsDocker.composeInstall")}</p>
      <ul className="mt-1 space-y-1 text-[12.5px]">
        {(
          [
            ["debian13", "sudo apt install docker-compose"],
            ["ubuntu", "sudo apt install docker-compose-v2"],
            ["dockerRepo", "sudo apt install docker-compose-plugin"],
          ] as const
        ).map(([k, cmd]) => (
          <li key={k}>
            {t(`settingsDocker.composeInstallOn.${k}`)} <code className="selectable rounded bg-code px-1 font-mono text-[11.5px]">{cmd}</code>
          </li>
        ))}
      </ul>
      <Button className="mt-5" onClick={() => void q.refetch()} disabled={q.isFetching}>
        {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
        {t("settingsDocker.composeCheck")}
      </Button>
    </div>
  );
}

function ComposePages() {
  const page = useComposeNav((s) => s.page);
  switch (page.kind) {
    case "detail":
      return <ProjectDetail key={page.name} name={page.name} tab={page.tab} logServices={page.logServices} />;
    case "new":
      return <NewProject />;
    default:
      return <ProjectsTable />;
  }
}

export function ComposeView() {
  return (
    <RequireConnection>
      <RequireCompose>
        <ComposePages />
      </RequireCompose>
    </RequireConnection>
  );
}
