import type { ReactNode } from "react";
import { useApi, useServerView } from "@/app/server";
import { RemotePanel } from "@/features/servers/RemotePanel";
import { useQuery } from "@tanstack/react-query";
import { ArrowRight, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { DiagnosisPanel } from "@/features/engine/DiagnosisPanel";
import { SourceLabel } from "@/features/engine/SourceLabel";
import { useEngine } from "@/features/engine/store";
import { formatBytes, numberFormat } from "@/shared/format";
import type { SystemOverview } from "@/shared/ipc/bindings/SystemOverview";
import { keys } from "@/shared/query";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";

function Spec({ label, children, mono }: { label: string; children: ReactNode; mono?: boolean }) {
  return (
    <>
      <dt className="text-ink-3">{label}</dt>
      <dd className={cx("selectable min-w-0 [overflow-wrap:anywhere]", mono && "font-mono text-[12px]")}>{children ?? "—"}</dd>
    </>
  );
}

function Section({ title, children, className }: { title: string; children: ReactNode; className?: string }) {
  return (
    <section className={cx("rounded-xl border border-line bg-surface px-5 py-4", className)}>
      <h2 className="mb-3 text-[13px] font-semibold text-ink-2">{title}</h2>
      {children}
    </section>
  );
}

/** Barra proporcional de estados de contenedores. */
function StateBar({ running, paused, stopped }: { running: number; paused: number; stopped: number }) {
  const total = running + paused + stopped;
  if (total === 0) return <div className="h-2 rounded-full bg-surface-sunk" />;
  const pct = (n: number) => `${(n / total) * 100}%`;
  return (
    <div className="flex h-2 overflow-hidden rounded-full bg-surface-sunk" aria-hidden>
      <span className="bg-ok" style={{ width: pct(running) }} />
      <span className="bg-warn" style={{ width: pct(paused) }} />
      <span className="bg-off" style={{ width: pct(stopped) }} />
    </div>
  );
}

function Connected({ data, refetch, fetching }: { data: SystemOverview; refetch: () => void; fetching: boolean }) {
  const { t, i18n } = useTranslation();
  const navigate = useApp((s) => s.navigate);
  const { info, version, endpoint } = data;
  const nf = numberFormat(i18n.language);
  const running = Number(info.containersRunning ?? 0);
  const paused = Number(info.containersPaused ?? 0);
  const stopped = Number(info.containersStopped ?? 0);

  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-end gap-x-8 gap-y-3">
        <div>
          <p className="text-[12.5px] text-ink-3">{t("dashboard.engine")}</p>
          <p className="text-[30px] font-semibold leading-none tracking-[-0.02em] tabular-nums">{version.version ?? info.serverVersion}</p>
        </div>
        <dl className="flex flex-wrap gap-x-6 gap-y-1 pb-0.5 text-[12.5px]">
          <div>
            <dt className="text-ink-3">{t("dashboard.api")}</dt>
            <dd className="tabular-nums">{version.apiVersion}</dd>
          </div>
          <div>
            <dt className="text-ink-3">{t("dashboard.socket")}</dt>
            <dd className="selectable font-mono text-[12px]">{endpoint.socketPath}</dd>
          </div>
          <div>
            <dt className="text-ink-3">{t("dashboard.source")}</dt>
            <dd>
              <SourceLabel source={endpoint.source} />
              {info.rootless && <span className="ml-1.5 rounded bg-beacon-soft px-1 text-[10.5px] font-semibold text-beacon-strong">{t("conn.rootless")}</span>}
            </dd>
          </div>
        </dl>
        <Button className="ml-auto" onClick={refetch} disabled={fetching}>
          {fetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("dashboard.refresh")}
        </Button>
      </header>

      <div className="grid gap-5 md:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
        <section className="rounded-xl border border-line bg-surface px-5 py-4">
          <div className="flex items-baseline justify-between">
            <h2 className="text-[13px] font-semibold text-ink-2">{t("dashboard.containers")}</h2>
            <button type="button" onClick={() => navigate("containers")} className="inline-flex items-center gap-1 text-[12.5px] text-beacon-strong hover:underline">
              {t("dashboard.viewContainers")} <ArrowRight size={13} />
            </button>
          </div>
          <p className="mt-2 text-[40px] font-semibold leading-none tracking-[-0.02em] tabular-nums">{nf.format(Number(info.containers ?? 0))}</p>
          <div className="mt-4">
            <StateBar running={running} paused={paused} stopped={stopped} />
          </div>
          <dl className="mt-3 flex flex-wrap gap-x-6 gap-y-1 text-[12.5px]">
            {(
              [
                ["running", running, "bg-ok"],
                ["paused", paused, "bg-warn"],
                ["stopped", stopped, "bg-off"],
              ] as const
            ).map(([k, n, c]) => (
              <div key={k} className="flex items-center gap-1.5">
                <span aria-hidden className={cx("h-2 w-2 rounded-full", c)} />
                <dt className="text-ink-2">{t(`dashboard.${k}`)}</dt>
                <dd className="font-medium tabular-nums">{nf.format(n)}</dd>
              </div>
            ))}
          </dl>
        </section>

        <section className="rounded-xl border border-line bg-surface px-5 py-4">
          <div className="flex items-baseline justify-between">
            <h2 className="text-[13px] font-semibold text-ink-2">{t("dashboard.images")}</h2>
            <button type="button" onClick={() => navigate("images")} className="inline-flex items-center gap-1 text-[12.5px] text-beacon-strong hover:underline">
              {t("dashboard.viewImages")} <ArrowRight size={13} />
            </button>
          </div>
          <p className="mt-2 text-[40px] font-semibold leading-none tracking-[-0.02em] tabular-nums">{nf.format(Number(info.images ?? 0))}</p>
          <p className="mt-2 text-[12.5px] text-ink-3">{t("dashboard.imagesHint")}</p>
        </section>
      </div>

      <div className="grid gap-5 md:grid-cols-2">
        <Section title={t("dashboard.host")}>
          <dl className="grid grid-cols-[auto_1fr] gap-x-5 gap-y-1.5 text-[13px]">
            <Spec label={t("dashboard.hostName")}>{info.name}</Spec>
            <Spec label={t("dashboard.os")}>{info.operatingSystem}</Spec>
            <Spec label={t("dashboard.kernel")} mono>
              {info.kernelVersion}
            </Spec>
            <Spec label={t("dashboard.arch")}>{info.architecture}</Spec>
            <Spec label={t("dashboard.cpus")}>{info.ncpu !== null ? nf.format(Number(info.ncpu)) : null}</Spec>
            <Spec label={t("dashboard.memory")}>{info.memTotal !== null ? formatBytes(info.memTotal, i18n.language) : null}</Spec>
          </dl>
        </Section>
        <Section title={t("dashboard.runtime")}>
          <dl className="grid grid-cols-[auto_1fr] gap-x-5 gap-y-1.5 text-[13px]">
            <Spec label={t("dashboard.storageDriver")}>{info.storageDriver}</Spec>
            <Spec label={t("dashboard.rootDir")} mono>
              {info.dockerRootDir}
            </Spec>
            <Spec label={t("dashboard.cgroup")}>{[info.cgroupDriver, info.cgroupVersion && `v${info.cgroupVersion}`].filter(Boolean).join(", ") || null}</Spec>
            <Spec label={t("dashboard.logging")}>{info.loggingDriver}</Spec>
            <Spec label={t("dashboard.go")}>{version.goVersion}</Spec>
            <Spec label={t("dashboard.minApi")}>{version.minApiVersion}</Spec>
          </dl>
          {info.swarmActive && <p className="mt-3 text-[12.5px] text-beacon-strong">{t("dashboard.swarm")}</p>}
        </Section>
      </div>

      {info.warnings.length > 0 && (
        <Section title={t("dashboard.warnings")}>
          <ul className="space-y-1.5 text-[12.5px]">
            {info.warnings.map((w) => (
              <li key={w} className="selectable flex gap-2 text-ink-2 [overflow-wrap:anywhere]">
                <span aria-hidden className="mt-[7px] h-1 w-1 shrink-0 rounded-full bg-warn" />
                {w}
              </li>
            ))}
          </ul>
        </Section>
      )}
    </div>
  );
}

export function DashboardView() {
  const api = useApi();
  const { t } = useTranslation();
  const { state, reconnecting, reconnect } = useEngine();
  const navigate = useApp((s) => s.navigate);
  const connected = state?.state === "connected";
  const remote = !!useServerView()?.remote;
  const q = useQuery({ queryKey: keys.overview, queryFn: api.engineOverview, enabled: connected });

  return (
    <div className="mx-auto max-w-[1080px] px-8 py-7">
      {remote && state?.state !== "connected" && state?.state !== "connecting" ? (
        <RemotePanel />
      ) : state === null || state.state === "connecting" || state.state === "idle" ? (
        <div className="flex items-center justify-center gap-2 py-24 text-[13px] text-ink-3">
          <Spinner /> {t("conn.connecting")}
        </div>
      ) : state.state === "disconnected" ? (
        <DiagnosisPanel diagnosis={state.diagnosis} onReconnect={() => void reconnect()} reconnecting={reconnecting} onConfigure={() => navigate("settings")} />
      ) : q.data ? (
        <>
          {q.error && (
            <div className="mb-4">
              <Banner tone="error">{t("dashboard.loadError", { message: (q.error as { message: string }).message })}</Banner>
            </div>
          )}
          <Connected data={q.data} refetch={() => void q.refetch()} fetching={q.isFetching} />
        </>
      ) : q.error ? (
        <Banner tone="error">{t("dashboard.loadError", { message: (q.error as { message: string }).message })}</Banner>
      ) : (
        <div className="flex items-center justify-center gap-2 py-24 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      )}
    </div>
  );
}
