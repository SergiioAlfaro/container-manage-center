import { useCallback, useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { HardDrive, Lock, PackageCheck, PackageX, ScanSearch, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { numberFormat } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { ComponentsReport } from "@/shared/ipc/bindings/ComponentsReport";
import type { PackageClass } from "@/shared/ipc/bindings/PackageClass";
import type { PackageEntry } from "@/shared/ipc/bindings/PackageEntry";
import type { RemovalPlan } from "@/shared/ipc/bindings/RemovalPlan";
import type { RemovalResult } from "@/shared/ipc/bindings/RemovalResult";
import { Button, CopyButton } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";

/** KiB → texto legible. */
export function formatKib(kib: number, lang: string): string {
  const nf = numberFormat(lang, { maximumFractionDigits: 1 });
  if (kib >= 1024 * 1024) return `${nf.format(kib / 1024 / 1024)} GiB`;
  if (kib >= 1024) return `${nf.format(kib / 1024)} MiB`;
  return `${nf.format(kib)} KiB`;
}
const formatBytes = (b: number, lang: string) => formatKib(b / 1024, lang);

/** Orden en que se agrupa lo que se conserva. */
const KEPT: PackageClass[] = ["shared", "manual", "preExisting", "uncertain", "protected"];

function Section({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <section className="rounded-xl border border-line bg-surface">
      <h3 className="flex items-center gap-2 border-b border-line px-5 py-3 text-[13.5px] font-semibold text-ink">
        {icon}
        {title}
      </h3>
      <div className="px-5 py-3">{children}</div>
    </section>
  );
}

function PackageLine({ p, lang, children }: { p: PackageEntry; lang: string; children?: React.ReactNode }) {
  const { t } = useTranslation();
  return (
    <div className="flex items-start gap-3 py-1.5 text-[13px]">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-baseline gap-x-2">
          <span className="selectable font-mono text-[12.5px] text-ink">{p.name}</span>
          <span className="selectable text-[11.5px] text-ink-3">{p.version}</span>
          {p.direct && <span className="rounded bg-surface-2 px-1.5 text-[11px] text-ink-2">{t("components.direct")}</span>}
        </div>
        {children}
      </div>
      <span className="shrink-0 tabular-nums text-[12px] text-ink-3">{formatKib(p.installedKib, lang)}</span>
    </div>
  );
}

export function ComponentsSettings() {
  const { t, i18n } = useTranslation();
  const lang = i18n.language;
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [plan, setPlan] = useState<RemovalPlan | null>(null);

  const inspect = useMutation({
    mutationFn: api.componentsInspect,
    onSuccess: (r) => {
      setSelected(r.own ? new Set([r.own.name]) : new Set());
      remove.reset();
    },
  });
  const planning = useMutation({ mutationFn: (pkgs: string[]) => api.componentsPlan(pkgs), onSuccess: setPlan });
  const remove = useMutation({
    mutationFn: (pkgs: string[]) => api.componentsRemove(pkgs),
    onSettled: () => setPlan(null),
  });
  const closePlan = useCallback(() => setPlan(null), []);

  const report: ComponentsReport | undefined = inspect.data;
  const removable = useMemo(() => report?.packages.filter((p) => p.class === "removable") ?? [], [report]);
  const own = report?.own ?? null;
  const ownSelected = own !== null && selected.has(own.name);
  const freed = [own, ...removable].filter((p): p is PackageEntry => !!p && selected.has(p.name)).reduce((n, p) => n + p.installedKib, 0);

  const toggle = (name: string, on: boolean) =>
    setSelected((s) => {
      const n = new Set(s);
      if (on) n.add(name);
      else n.delete(name);
      // Las dependencias solo se pueden quitar junto con la aplicación.
      if (own && name === own.name && !on) removable.forEach((p) => n.delete(p.name));
      if (own && on && name !== own.name) n.add(own.name);
      return n;
    });
  const preset = (withDeps: boolean) => {
    if (!own) return;
    setSelected(new Set([own.name, ...(withDeps ? removable.map((p) => p.name) : [])]));
  };

  const result: RemovalResult | undefined = remove.data;

  return (
    <div className="space-y-4">
      <p className="text-[13px] text-ink-2">{t("components.intro")}</p>
      <div className="flex flex-wrap items-center gap-3">
        <Button variant={report ? "quiet" : "primary"} loading={inspect.isPending} onClick={() => inspect.mutate()}>
          {!inspect.isPending && <ScanSearch size={14} />}
          {report ? t("components.reanalyze") : t("components.analyze")}
        </Button>
        <span className="text-[12px] text-ink-3">{t("components.analyzeHint")}</span>
      </div>
      {inspect.error && <Banner tone="error">{(inspect.error as { message: string }).message}</Banner>}

      {result && (
        <Banner tone="info">
          <strong className="text-ink">{t("components.done", { count: result.removed.length })}</strong>{" "}
          {result.removedSelf && t("components.doneSelf")}
          {result.stillInstalled.length > 0 && <> {t("components.stillInstalled", { list: result.stillInstalled.join(", ") })}</>}
        </Banner>
      )}
      {remove.error && <Banner tone="error">{(remove.error as { message: string }).message}</Banner>}

      {report && !report.supported && <Banner tone="warn">{t("components.unsupported")}</Banner>}
      {report?.supported && !own && <Banner tone="info">{t("components.notPackaged")}</Banner>}

      {report && own && (
        <>
          {report.history !== "found" && <Banner tone="warn">{t(`components.history.${report.history}`)}</Banner>}

          <Section title={t("components.ownTitle")} icon={<PackageX size={15} className="text-ink-3" aria-hidden />}>
            <PackageLine p={own} lang={lang}>
              <p className="text-[12px] text-ink-3">{t("components.class.own")}</p>
            </PackageLine>
            <div className="mt-2">
              <Checkbox checked={ownSelected} onChange={(v) => toggle(own.name, v)} label={t("components.removeApp")} />
            </div>
          </Section>

          <Section title={t("components.removableTitle", { count: removable.length })} icon={<PackageCheck size={15} className="text-ink-3" aria-hidden />}>
            {removable.length === 0 ? (
              <p className="text-[13px] text-ink-3">{t("components.noneRemovable")}</p>
            ) : (
              <>
                <p className="mb-2 text-[12px] text-ink-3">{t("components.class.removable")}</p>
                <ul className="divide-y divide-line">
                  {removable.map((p) => (
                    <li key={p.name} className="flex items-start gap-2">
                      <div className="pt-2">
                        <input
                          type="checkbox"
                          aria-label={t("components.removePackage", { name: p.name })}
                          checked={selected.has(p.name)}
                          onChange={(e) => toggle(p.name, e.target.checked)}
                          className="h-4 w-4 accent-[var(--color-beacon)]"
                        />
                      </div>
                      <div className="min-w-0 flex-1">
                        <PackageLine p={p} lang={lang} />
                      </div>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </Section>

          <div className="flex flex-wrap items-center gap-2 rounded-xl border border-line bg-surface-2 px-5 py-3">
            <Button size="sm" onClick={() => preset(false)}>
              {t("components.presetApp")}
            </Button>
            <Button size="sm" disabled={removable.length === 0} onClick={() => preset(true)}>
              {t("components.presetAll")}
            </Button>
            <span className="ml-auto text-[12.5px] text-ink-2" data-testid="components-summary">
              {selected.size > 0 ? t("components.summary", { count: selected.size, size: formatKib(freed, lang) }) : t("components.nothingSelected")}
            </span>
            <Button variant="danger" disabled={selected.size === 0 || !ownSelected} loading={planning.isPending || remove.isPending} onClick={() => planning.mutate([...selected])}>
              {t("components.review")}
            </Button>
          </div>
          {planning.error && <Banner tone="error">{(planning.error as { message: string }).message}</Banner>}

          <Section title={t("components.keptTitle")} icon={<Lock size={15} className="text-ink-3" aria-hidden />}>
            {KEPT.map((c) => {
              const list = report.packages.filter((p) => p.class === c);
              if (list.length === 0) return null;
              return (
                <details key={c} className="group border-b border-line py-2 last:border-0" open={c === "shared" && list.length <= 8}>
                  <summary className="cursor-pointer rounded text-[13px] text-ink hover:text-beacon-strong">
                    {t(`components.group.${c}`, { count: list.length })}
                  </summary>
                  <p className="mt-1 text-[12px] text-ink-3">{t(`components.class.${c}`)}</p>
                  <ul className="mt-1">
                    {list.map((p) => (
                      <li key={p.name}>
                        <PackageLine p={p} lang={lang}>
                          {p.usedBy.length > 0 && (
                            <p className="text-[12px] text-ink-3">{t("components.usedBy", { list: p.usedBy.slice(0, 6).join(", "), count: p.usedBy.length })}</p>
                          )}
                        </PackageLine>
                      </li>
                    ))}
                  </ul>
                </details>
              );
            })}
          </Section>
        </>
      )}

      {report && (
        <>
          <Section title={t("components.externalTitle")} icon={<ShieldCheck size={15} className="text-ink-3" aria-hidden />}>
            <p className="mb-2 text-[12px] text-ink-3">{t("components.externalHint")}</p>
            <ul className="space-y-1.5" data-testid="external-tools">
              {report.external.map((e) => (
                <li key={e.kind} className="flex flex-wrap items-baseline gap-x-2 text-[13px]">
                  <span className="font-medium text-ink">{t(`components.external.${e.kind}`)}</span>
                  <span className="selectable font-mono text-[12px] text-ink-2">
                    {e.packages.length ? e.packages.join(", ") : (e.binary ?? t("components.notFound"))}
                  </span>
                  <span className="ml-auto text-[12px] text-ok">{t("components.alwaysKept")}</span>
                </li>
              ))}
            </ul>
          </Section>
          <Section title={t("components.dataTitle")} icon={<HardDrive size={15} className="text-ink-3" aria-hidden />}>
            <p className="mb-2 text-[12px] text-ink-3">{t("components.dataHint")}</p>
            <ul className="space-y-1.5">
              {report.userData.map((u) => (
                <li key={u.path} className="flex items-baseline gap-2 text-[13px]">
                  <span className="w-40 shrink-0 text-ink-2">{t(`components.data.${u.kind}`)}</span>
                  <code className="selectable min-w-0 flex-1 truncate font-mono text-[12px] text-ink-2" title={u.path}>
                    {u.path}
                  </code>
                  <span className="shrink-0 tabular-nums text-[12px] text-ink-3">{u.exists ? formatBytes(Number(u.bytes), lang) : t("components.absent")}</span>
                </li>
              ))}
            </ul>
          </Section>
          {report.simulation && (
            <details className="rounded-xl border border-line bg-surface px-5 py-3">
              <summary className="cursor-pointer text-[13px] text-ink hover:text-beacon-strong">{t("components.fullSimulation")}</summary>
              <p className="mt-2 text-[12px] text-ink-3">{t("components.fullSimulationHint")}</p>
              <code className="selectable mt-2 block font-mono text-[12px] text-ink-2">{report.simulation.command}</code>
              <pre className="selectable mt-2 max-h-64 overflow-auto rounded-md bg-code p-3 font-mono text-[11.5px] text-ink-2">{report.simulation.output}</pre>
            </details>
          )}
        </>
      )}

      {plan && (
        <Dialog
          target={false}
          title={t("components.confirmTitle")}
          onClose={closePlan}
          footer={
            <>
              <Button onClick={closePlan}>{t("common.cancel")}</Button>
              <Button variant="danger" loading={remove.isPending} disabled={plan.extra.length > 0 || !report?.elevation} onClick={() => remove.mutate(plan.packages)}>
                {t("components.confirm")}
              </Button>
            </>
          }
        >
          <p>{t("components.confirmBody", { count: plan.packages.length, size: formatKib(plan.freedKib, lang) })}</p>
          <ul className="max-h-32 overflow-auto rounded-md border border-line px-3 py-2 font-mono text-[12px]" data-testid="plan-packages">
            {plan.packages.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
          {plan.extra.length > 0 && <Banner tone="error">{t("components.extra", { list: plan.extra.join(", ") })}</Banner>}
          <div>
            <p className="text-[12.5px] font-medium text-ink-2">{t("components.command")}</p>
            <div className="mt-1 flex items-start gap-2">
              <code className="selectable min-w-0 flex-1 break-all rounded-md bg-code px-2 py-1.5 font-mono text-[12px]">{plan.command}</code>
              <CopyButton text={plan.command} compact />
            </div>
            <p className="mt-1 text-[12px] text-ink-3">{report?.elevation ? t("components.elevationHint") : t("components.noElevation")}</p>
          </div>
          <details>
            <summary className="cursor-pointer text-[12.5px] text-ink-2 hover:text-ink">{t("components.simulation")}</summary>
            <code className="selectable mt-1 block font-mono text-[11.5px] text-ink-3">{plan.simulation.command}</code>
            <pre className="selectable mt-1 max-h-40 overflow-auto rounded-md bg-code p-2 font-mono text-[11px] text-ink-2">{plan.simulation.output}</pre>
          </details>
          <p className="text-[12px] text-ink-3">{t("components.keptNote")}</p>
          {plan.removesSelf && <Banner tone="warn">{t("components.selfNote")}</Banner>}
        </Dialog>
      )}
      {remove.isPending && (
        <p role="status" className="flex items-center gap-2 text-[13px] text-ink-2">
          <Spinner size={14} /> {t("components.removing")}
        </p>
      )}
    </div>
  );
}
