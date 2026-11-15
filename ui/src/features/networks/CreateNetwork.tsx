// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useRef, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { IpamPool } from "@/shared/ipc/bindings/IpamPool";
import type { KeyValue } from "@/shared/ipc/bindings/KeyValue";
import type { NetworkPreview } from "@/shared/ipc/bindings/NetworkPreview";
import type { NetworkSpec } from "@/shared/ipc/bindings/NetworkSpec";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import { Button, CopyButton, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { Field, Group, IssueList, RowList, TextInput, useIssueText } from "@/features/containers/create/fields";
import { issuesFor } from "@/features/containers/specModel";
import { useInvalidateNetworks } from "./dialogs";
import { emptyNetworkSpec } from "./model";
import { useNetworksNav } from "./store";

type DriverChoice = "bridge" | "macvlan" | "ipvlan" | "overlay" | "other";
const KNOWN: DriverChoice[] = ["bridge", "macvlan", "ipvlan", "overlay"];

function KvList({ rows, onChange, field, issues, addLabel, empty, keyPh, valuePh }: { rows: KeyValue[]; onChange: (v: KeyValue[]) => void; field: string; issues: SpecIssue[]; addLabel: string; empty: string; keyPh: string; valuePh: string }) {
  const { t } = useTranslation();
  return (
    <RowList<KeyValue>
      rows={rows}
      onChange={onChange}
      make={() => ({ key: "", value: "" })}
      addLabel={addLabel}
      removeLabel={(i) => t("create.remove.row", { n: i + 1 })}
      empty={empty}
      render={(kv, set, i) => (
        <div>
          <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] gap-2">
            <TextInput aria-label={t("create.f.key", { n: i + 1 })} invalid={issuesFor(issues, `${field}.${i}.key`).some((x) => x.severity === "error")} value={kv.key} onChange={(e) => set({ ...kv, key: e.target.value })} placeholder={keyPh} />
            <TextInput aria-label={t("create.f.value", { n: i + 1 })} value={kv.value} onChange={(e) => set({ ...kv, value: e.target.value })} placeholder={valuePh} />
          </div>
          <IssueList issues={issuesFor(issues, `${field}.${i}.key`)} />
        </div>
      )}
    />
  );
}

function PoolFields({ family, pool, onChange, issues, ph }: { family: "ipv4" | "ipv6"; pool: IpamPool; onChange: (p: IpamPool) => void; issues: SpecIssue[]; ph: { subnet: string; gateway: string; range: string } }) {
  const { t } = useTranslation();
  const of = (f: string) => issuesFor(issues, `${family}.${f}`);
  return (
    <div className="grid gap-4 md:grid-cols-3">
      <Field label={t("netCreate.f.subnet")} issues={of("subnet")} hint={t("netCreate.h.subnet")}>
        {(id, inv) => <TextInput id={id} invalid={inv} value={pool.subnet} onChange={(e) => onChange({ ...pool, subnet: e.target.value })} placeholder={ph.subnet} />}
      </Field>
      <Field label={t("netCreate.f.gateway")} issues={of("gateway")} hint={t("netCreate.h.gateway")}>
        {(id, inv) => <TextInput id={id} invalid={inv} value={pool.gateway} onChange={(e) => onChange({ ...pool, gateway: e.target.value })} placeholder={ph.gateway} />}
      </Field>
      <Field label={t("netCreate.f.ipRange")} issues={of("ipRange")} hint={t("netCreate.h.ipRange")}>
        {(id, inv) => <TextInput id={id} invalid={inv} value={pool.ipRange} onChange={(e) => onChange({ ...pool, ipRange: e.target.value })} placeholder={ph.range} />}
      </Field>
    </div>
  );
}

/** Asistente de creación de redes. La validación, el cuerpo y el comando los genera el backend. */
export function CreateNetwork() {
  const api = useApi();
  const { t } = useTranslation();
  const nav = useNetworksNav();
  const invalidate = useInvalidateNetworks();
  const issueText = useIssueText();
  const [spec, setSpec] = useState<NetworkSpec>(emptyNetworkSpec);
  const [choice, setChoice] = useState<DriverChoice>("bridge");
  const [preview, setPreview] = useState<NetworkPreview | null>(null);
  const set = <K extends keyof NetworkSpec>(k: K, v: NetworkSpec[K]) => setSpec((s) => ({ ...s, [k]: v }));

  const seq = useRef(0);
  useEffect(() => {
    const n = ++seq.current;
    const timer = window.setTimeout(() => {
      api
        .networkPreview(spec)
        .then((p) => n === seq.current && setPreview(p))
        .catch(() => {});
    }, 200);
    return () => window.clearTimeout(timer);
  }, [spec]);

  const issues = preview?.issues ?? [];
  const of = (f: string) => issuesFor(issues, f);
  const errors = issues.filter((i) => i.severity === "error");
  const warnings = issues.filter((i) => i.severity === "warning");

  const m = useMutation({
    mutationFn: () => api.networkCreate(spec),
    onSuccess: (r) => {
      invalidate();
      nav.showDetail(r.id, spec.name.trim());
    },
  });

  const pickDriver = (c: DriverChoice) => {
    setChoice(c);
    if (c !== "other") set("driver", c);
    else if (KNOWN.includes(spec.driver as DriverChoice)) set("driver", "");
  };

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("netDetail.back")}
        </Button>
        <h2 className="text-[14.5px] font-semibold">{t("netCreate.title")}</h2>
      </div>
      <div className="grid flex-1 gap-6 px-8 py-5 xl:grid-cols-[minmax(0,1fr)_400px]">
        <div className="min-w-0 space-y-6">
          <Group title={t("netCreate.g.general")}>
            <Field label={t("netCreate.f.name")} issues={of("name")} hint={t("netCreate.h.name")}>
              {(id, inv) => <TextInput id={id} invalid={inv} value={spec.name} onChange={(e) => set("name", e.target.value)} placeholder="mi-red" autoFocus />}
            </Field>
            <div>
              <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("netCreate.f.driver")}</p>
              <Segmented<DriverChoice>
                label={t("netCreate.f.driver")}
                value={choice}
                onChange={pickDriver}
                options={[...KNOWN, "other" as const].map((d) => ({ value: d, label: d === "other" ? t("netCreate.otherDriver") : d }))}
              />
              <p className="mt-1 text-[12px] text-ink-3">{t(`netCreate.h.driver.${choice}`)}</p>
              {choice === "other" && (
                <div className="mt-2 max-w-[320px]">
                  <Field label={t("netCreate.f.driverName")} issues={of("driver")}>
                    {(id, inv) => <TextInput id={id} invalid={inv} value={spec.driver} onChange={(e) => set("driver", e.target.value)} placeholder="weaveworks/net-plugin" />}
                  </Field>
                </div>
              )}
              {choice !== "other" && <IssueList issues={of("driver")} />}
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              <Checkbox checked={spec.internal} onChange={(v) => set("internal", v)} label={t("netCreate.f.internal")} hint={t("netCreate.h.internal")} />
              <Checkbox checked={spec.attachable} onChange={(v) => set("attachable", v)} label={t("netCreate.f.attachable")} hint={t("netCreate.h.attachable")} />
            </div>
          </Group>

          <Group title={t("netCreate.g.ipv4")} description={t("netCreate.g.ipv4Hint")}>
            <PoolFields family="ipv4" pool={spec.ipv4} onChange={(p) => set("ipv4", p)} issues={issues} ph={{ subnet: "172.30.0.0/24", gateway: "172.30.0.1", range: "172.30.0.128/25" }} />
          </Group>

          <Group title={t("netCreate.g.ipv6")}>
            <Checkbox checked={spec.enableIpv6} onChange={(v) => set("enableIpv6", v)} label={t("netCreate.f.ipv6")} hint={t("netCreate.h.ipv6")} />
            {(spec.enableIpv6 || of("ipv6.subnet").length > 0) && (
              <PoolFields family="ipv6" pool={spec.ipv6} onChange={(p) => set("ipv6", p)} issues={issues} ph={{ subnet: "fd00:cafe::/64", gateway: "fd00:cafe::1", range: "fd00:cafe::/80" }} />
            )}
          </Group>

          <Group title={t("netCreate.g.options")} description={t("netCreate.g.optionsHint")}>
            <KvList rows={spec.options} onChange={(v) => set("options", v)} field="options" issues={issues} addLabel={t("netCreate.addOption")} empty={t("netCreate.noOptions")} keyPh={choice === "macvlan" || choice === "ipvlan" ? "parent" : "com.docker.network.driver.mtu"} valuePh={choice === "macvlan" || choice === "ipvlan" ? "eth0" : "1400"} />
          </Group>
          <Group title={t("netCreate.g.labels")}>
            <KvList rows={spec.labels} onChange={(v) => set("labels", v)} field="labels" issues={issues} addLabel={t("create.add.label")} empty={t("create.empty.labels")} keyPh="com.example.proyecto" valuePh="tienda" />
          </Group>
        </div>

        <aside className="min-w-0 space-y-4 xl:sticky xl:top-5 xl:self-start">
          <div className="rounded-xl border border-line bg-surface">
            <div className="flex items-center gap-2 border-b border-line px-4 py-2.5">
              <h3 className="flex-1 text-[13px] font-semibold">{t("create.equivalent")}</h3>
              {preview && <CopyButton text={preview.command} />}
            </div>
            <div className="space-y-2 px-4 py-3">
              {preview ? (
                <>
                  <pre data-testid="network-command" className="selectable max-h-[200px] overflow-auto rounded-md bg-code p-2.5 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap [overflow-wrap:anywhere]">
                    {preview.command}
                  </pre>
                  <p className="font-mono text-[11px] text-ink-3">{preview.request}</p>
                  <details className="text-[12px]">
                    <summary className="cursor-pointer text-ink-2">{t("netCreate.body")}</summary>
                    <pre className="selectable mt-1.5 max-h-[220px] overflow-auto rounded-md bg-code p-2.5 font-mono text-[11px]">{preview.body}</pre>
                  </details>
                </>
              ) : (
                <div className="flex items-center gap-2 py-3 text-[12.5px] text-ink-3">
                  <Spinner size={14} /> {t("create.previewing")}
                </div>
              )}
            </div>
          </div>

          <div className="rounded-xl border border-line bg-surface px-4 py-3">
            <h3 className="mb-1.5 text-[13px] font-semibold">{t("create.review")}</h3>
            {errors.length === 0 && warnings.length === 0 ? (
              <p className="text-[12.5px] text-ok">{t("netCreate.ready")}</p>
            ) : (
              <ul className="space-y-1 text-[12.5px]">
                {[...errors, ...warnings].map((i, n) => (
                  <li key={n} className={i.severity === "error" ? "text-err" : "text-warn"}>
                    {issueText(i)}
                  </li>
                ))}
              </ul>
            )}
          </div>

          {m.error && <Banner tone="error">{t("netOps.failed", { message: (m.error as { message: string }).message })}</Banner>}
          <Button variant="primary" className="w-full justify-center" disabled={!preview || errors.length > 0 || m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Plus size={15} />}
            {t("netCreate.submit")}
          </Button>
        </aside>
      </div>
    </div>
  );
}
