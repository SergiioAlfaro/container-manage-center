import { useState, type ReactNode } from "react";
import { Eye, EyeOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatBytes, shortId, uniquePorts } from "@/shared/format";
import type { ContainerDetails } from "@/shared/ipc/bindings/ContainerDetails";
import { cx } from "@/shared/ui/controls";
import { joinArgs } from "../shellwords";
import { isSecretKey } from "../specModel";

function Section({ title, children, count }: { title: string; children: ReactNode; count?: number }) {
  return (
    <section className="rounded-xl border border-line bg-surface">
      <h3 className="flex items-center gap-2 border-b border-line px-4 py-2.5 text-[13px] font-semibold">
        {title}
        {count !== undefined && <span className="text-[12px] font-normal text-ink-3 tabular-nums">{count}</span>}
      </h3>
      <div className="px-4 py-3">{children}</div>
    </section>
  );
}

function Rows({ rows }: { rows: Array<[string, ReactNode]> }) {
  return (
    <dl className="grid grid-cols-[minmax(120px,max-content)_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-[13px]">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-ink-3">{k}</dt>
          <dd className="selectable min-w-0 [overflow-wrap:anywhere]">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

const mono = (s: string) => <span className="font-mono text-[12.5px]">{s}</span>;
const none = <span className="text-ink-3">—</span>;

export function formatDockerTime(s: string | null, lang: string): string | null {
  if (!s || s.startsWith("0001-")) return null;
  const d = new Date(s);
  return Number.isNaN(d.getTime()) ? s : d.toLocaleString(lang === "es" ? "es-ES" : "en-US");
}

export function SummaryPanel({ d }: { d: ContainerDetails }) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language;
  const [reveal, setReveal] = useState(false);
  const secrets = d.env.filter((e) => isSecretKey(e.key)).length;
  const ports = d.ports.filter((p) => p.publicPort !== null);
  const exposed = d.ports.filter((p) => p.publicPort === null);
  const yes = t("common.yes");
  const no = t("common.no");
  const restart = d.restart.name === "onFailure" && d.restart.maxRetries > 0 ? `${t(`restart.${d.restart.name}`)} (${t("detail.maxRetries", { count: d.restart.maxRetries })})` : t(`restart.${d.restart.name}`);
  const labels = Object.entries(d.labels).sort(([a], [b]) => a.localeCompare(b));

  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Section title={t("detail.s.general")}>
        <Rows
          rows={[
            [t("detail.f.id"), mono(d.id)],
            [t("detail.f.image"), <>{mono(d.image)} <span className="ml-1 font-mono text-[11.5px] text-ink-3">{shortId(d.imageId)}</span></>],
            [t("detail.f.created"), formatDockerTime(d.created, lang) ?? none],
            [t("detail.f.started"), formatDockerTime(d.startedAt, lang) ?? none],
            [t("detail.f.finished"), formatDockerTime(d.finishedAt, lang) ?? none],
            [t("detail.f.exitCode"), d.running || d.exitCode === null ? none : mono(String(d.exitCode))],
            ...(d.oomKilled ? ([[t("detail.f.oom"), <span className="text-err">{yes}</span>]] as Array<[string, ReactNode]>) : []),
            ...(d.error ? ([[t("detail.f.error"), <span className="text-err">{d.error}</span>]] as Array<[string, ReactNode]>) : []),
            ...(d.health ? ([[t("detail.f.health"), d.health]] as Array<[string, ReactNode]>) : []),
            [t("detail.f.pid"), d.pid ? mono(String(d.pid)) : none],
            [t("detail.f.restartCount"), mono(String(d.restartCount))],
            ...(d.composeProject ? ([[t("detail.f.compose"), d.composeProject]] as Array<[string, ReactNode]>) : []),
          ]}
        />
      </Section>

      <Section title={t("detail.s.process")}>
        <Rows
          rows={[
            [t("detail.f.entrypoint"), d.entrypoint.length ? mono(joinArgs(d.entrypoint)) : none],
            [t("detail.f.command"), d.command.length ? mono(joinArgs(d.command)) : none],
            [t("detail.f.workingDir"), d.workingDir ? mono(d.workingDir) : none],
            [t("detail.f.user"), d.user ? mono(d.user) : <span className="text-ink-3">{t("detail.imageDefault")}</span>],
            [t("detail.f.hostname"), mono(d.hostname)],
            [t("detail.f.tty"), `${d.tty ? yes : no} · ${t("detail.f.stdin")}: ${d.openStdin ? yes : no}`],
          ]}
        />
      </Section>

      <Section title={t("detail.s.ports")} count={ports.length}>
        {ports.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noPorts")}</p>
        ) : (
          <ul className="selectable space-y-0.5 font-mono text-[12.5px]">
            {uniquePorts(ports).map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        )}
        {exposed.length > 0 && <p className="mt-2 text-[12px] text-ink-3">{t("detail.exposedOnly", { ports: [...new Set(exposed.map((p) => `${p.privatePort}/${p.protocol}`))].join(", ") })}</p>}
      </Section>

      <Section title={t("detail.s.restartResources")}>
        <Rows
          rows={[
            [t("detail.f.restart"), restart],
            [t("detail.f.cpus"), d.cpus === null ? <span className="text-ink-3">{t("detail.unlimited")}</span> : mono(String(d.cpus))],
            [t("detail.f.memory"), d.memoryBytes === null ? <span className="text-ink-3">{t("detail.unlimited")}</span> : formatBytes(d.memoryBytes, lang)],
            [t("detail.f.memoryReservation"), d.memoryReservationBytes === null ? none : formatBytes(d.memoryReservationBytes, lang)],
            [
              t("detail.f.flags"),
              [d.privileged && "--privileged", d.autoRemove && "--rm", d.init && "--init", d.readOnlyRootfs && "--read-only"].filter(Boolean).join(" ") || none,
            ],
          ]}
        />
      </Section>

      <Section title={t("detail.s.env")} count={d.env.length}>
        {secrets > 0 && (
          <button type="button" onClick={() => setReveal((r) => !r)} className="mb-2 inline-flex items-center gap-1.5 text-[12.5px] text-beacon-strong hover:underline">
            {reveal ? <EyeOff size={14} /> : <Eye size={14} />}
            {reveal ? t("detail.hideSecrets") : t("detail.revealSecrets", { count: secrets })}
          </button>
        )}
        {d.env.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noEnv")}</p>
        ) : (
          <ul className="selectable space-y-0.5 font-mono text-[12px] [overflow-wrap:anywhere]">
            {d.env.map((e, i) => (
              <li key={`${e.key}-${i}`}>
                <span className="text-ink-2">{e.key}</span>=
                {isSecretKey(e.key) && !reveal ? <span className="text-ink-3" aria-label={t("detail.hidden")}>••••••••</span> : e.value}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t("detail.s.networks")} count={d.networks.length}>
        <p className="mb-2 text-[12.5px] text-ink-3">{t("detail.networkMode", { mode: d.networkMode })}</p>
        {d.networks.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noNetworks")}</p>
        ) : (
          <table className="w-full text-left text-[12.5px]">
            <thead className="text-ink-3">
              <tr>
                <th className="py-1 font-medium">{t("detail.f.network")}</th>
                <th className="py-1 font-medium">IP</th>
                <th className="py-1 font-medium">{t("detail.f.gateway")}</th>
              </tr>
            </thead>
            <tbody className="selectable font-mono">
              {d.networks.map((n) => (
                <tr key={n.name}>
                  <td className="py-0.5 pr-2">{n.name}</td>
                  <td className="py-0.5 pr-2">{n.ipAddress || "—"}</td>
                  <td className="py-0.5">{n.gateway || "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>

      <Section title={t("detail.s.mounts")} count={d.mounts.length}>
        {d.mounts.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noMounts")}</p>
        ) : (
          <ul className="space-y-1.5 text-[12.5px]">
            {d.mounts.map((m) => (
              <li key={m.destination} className="selectable font-mono [overflow-wrap:anywhere]">
                <span className="mr-1.5 rounded bg-surface-sunk px-1.5 font-sans text-[11.5px] text-ink-2">{m.kind}</span>
                {m.kind === "volume" ? m.name : m.source} → {m.destination}
                {m.readOnly && <span className="ml-1.5 font-sans text-[11.5px] text-warn">{t("detail.readOnly")}</span>}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t("detail.s.labels")} count={labels.length}>
        {labels.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noLabels")}</p>
        ) : (
          <ul className={cx("selectable space-y-0.5 font-mono text-[12px] [overflow-wrap:anywhere]")}>
            {labels.map(([k, v]) => (
              <li key={k}>
                <span className="text-ink-2">{k}</span>={v}
              </li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}
