// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useRef, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { KeyValue } from "@/shared/ipc/bindings/KeyValue";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import type { VolumePreview } from "@/shared/ipc/bindings/VolumePreview";
import type { VolumeSpec } from "@/shared/ipc/bindings/VolumeSpec";
import { keys } from "@/shared/query";
import { Button, CopyButton, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { Field, Group, IssueList, RowList, TextInput, useIssueText } from "@/features/containers/create/fields";
import { issuesFor } from "@/features/containers/specModel";
import { useInvalidateVolumes } from "./dialogs";
import { emptyVolumeSpec } from "./model";
import { useVolumesNav } from "./store";

type Preset = "plain" | "tmpfs" | "nfs" | "bind" | "other";

/** Opciones del driver local para casos habituales (equivalen a `mount -t …`). */
function presetOpts(p: Preset): KeyValue[] {
  switch (p) {
    case "tmpfs":
      return [
        { key: "type", value: "tmpfs" },
        { key: "device", value: "tmpfs" },
        { key: "o", value: "size=100m" },
      ];
    case "nfs":
      return [
        { key: "type", value: "nfs" },
        { key: "o", value: "addr=192.168.1.10,rw,nfsvers=4" },
        { key: "device", value: ":/ruta/exportada" },
      ];
    case "bind":
      return [
        { key: "type", value: "none" },
        { key: "o", value: "bind" },
        { key: "device", value: "/ruta/del/equipo" },
      ];
    default:
      return [];
  }
}

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
          <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)] gap-2">
            <TextInput aria-label={t("create.f.key", { n: i + 1 })} invalid={issuesFor(issues, `${field}.${i}.key`).some((x) => x.severity === "error")} value={kv.key} onChange={(e) => set({ ...kv, key: e.target.value })} placeholder={keyPh} />
            <TextInput aria-label={t("create.f.value", { n: i + 1 })} value={kv.value} onChange={(e) => set({ ...kv, value: e.target.value })} placeholder={valuePh} />
          </div>
          <IssueList issues={issuesFor(issues, `${field}.${i}.key`)} />
        </div>
      )}
    />
  );
}

/** Asistente de creación de volúmenes. Validación, cuerpo y comando los genera el backend. */
export function CreateVolume() {
  const api = useApi();
  const { t } = useTranslation();
  const nav = useVolumesNav();
  const invalidate = useInvalidateVolumes();
  const issueText = useIssueText();
  const [spec, setSpec] = useState<VolumeSpec>(emptyVolumeSpec);
  const [preset, setPreset] = useState<Preset>("plain");
  const [preview, setPreview] = useState<VolumePreview | null>(null);
  const [serverIssues, setServerIssues] = useState<SpecIssue[]>([]);
  const set = <K extends keyof VolumeSpec>(k: K, v: VolumeSpec[K]) => {
    setServerIssues([]);
    setSpec((s) => ({ ...s, [k]: v }));
  };

  const seq = useRef(0);
  useEffect(() => {
    const n = ++seq.current;
    const timer = window.setTimeout(() => {
      api
        .volumePreview(spec)
        .then((p) => n === seq.current && setPreview(p))
        .catch(() => {});
    }, 200);
    return () => window.clearTimeout(timer);
  }, [spec]);

  // Aviso inmediato con la lista actual; el backend lo vuelve a comprobar al crear.
  const existing = useQuery({ queryKey: keys.volumes, queryFn: api.volumesList, refetchOnMount: "always" });
  const name = spec.name.trim();
  const taken: SpecIssue[] =
    name && existing.data?.some((v) => v.name === name) && !serverIssues.some((i) => i.code === "volumeName.exists")
      ? [{ severity: "error", field: "name", code: "volumeName.exists", params: { value: name } }]
      : [];
  // Los problemas que solo detecta el backend al crear (p. ej. nombre ya existente) se suman a la vista previa.
  const issues = [...(preview?.issues ?? []), ...taken, ...serverIssues];
  const of = (f: string) => issuesFor(issues, f);
  const errors = issues.filter((i) => i.severity === "error");
  const warnings = issues.filter((i) => i.severity === "warning");

  const m = useMutation({
    mutationFn: () => api.volumeCreate(spec),
    onSuccess: (v) => {
      invalidate();
      nav.showDetail(v.name);
    },
    onError: (e) => {
      const ce = e as { code?: string; issues?: SpecIssue[] | null };
      if (ce.code === "engine.invalidSpec" && ce.issues) setServerIssues(ce.issues.filter((i) => !(preview?.issues ?? []).some((p) => p.code === i.code && p.field === i.field)));
    },
  });

  const pick = (p: Preset) => {
    setPreset(p);
    if (p === "other") set("driver", "");
    else {
      setSpec((s) => ({ ...s, driver: "local", driverOpts: presetOpts(p) }));
      setServerIssues([]);
    }
  };

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("volDetail.back")}
        </Button>
        <h2 className="text-[14.5px] font-semibold">{t("volCreate.title")}</h2>
      </div>
      <div className="grid flex-1 gap-6 px-8 py-5 xl:grid-cols-[minmax(0,1fr)_400px]">
        <div className="min-w-0 space-y-6">
          <Group title={t("volCreate.g.general")}>
            <Field label={t("volCreate.f.name")} issues={of("name")} hint={t("volCreate.h.name")}>
              {(id, inv) => <TextInput id={id} invalid={inv} value={spec.name} onChange={(e) => set("name", e.target.value)} placeholder={t("volCreate.ph.name")} autoFocus />}
            </Field>
            <div>
              <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("volCreate.f.kind")}</p>
              <Segmented<Preset>
                label={t("volCreate.f.kind")}
                value={preset}
                onChange={pick}
                options={(["plain", "tmpfs", "nfs", "bind", "other"] as Preset[]).map((p) => ({ value: p, label: t(`volCreate.preset.${p}`) }))}
              />
              <p className="mt-1 text-[12px] text-ink-3">{t(`volCreate.h.preset.${preset}`)}</p>
              {preset === "other" && (
                <div className="mt-2 max-w-[320px]">
                  <Field label={t("netCreate.f.driverName")} issues={of("driver")}>
                    {(id, inv) => <TextInput id={id} invalid={inv} value={spec.driver} onChange={(e) => set("driver", e.target.value)} placeholder="vieux/sshfs" />}
                  </Field>
                </div>
              )}
            </div>
          </Group>
          <Group title={t("volCreate.g.options")} description={t("volCreate.g.optionsHint")}>
            <IssueList issues={of("driverOpts")} />
            <KvList rows={spec.driverOpts} onChange={(v) => set("driverOpts", v)} field="driverOpts" issues={issues} addLabel={t("netCreate.addOption")} empty={t("netCreate.noOptions")} keyPh="type" valuePh="tmpfs" />
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
                  <pre data-testid="volume-command" className="selectable max-h-[200px] overflow-auto rounded-md bg-code p-2.5 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap [overflow-wrap:anywhere]">
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
              <p className="text-[12.5px] text-ok">{spec.name.trim() ? t("netCreate.ready") : t("volCreate.readyAnonymous")}</p>
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
          {m.error && (m.error as { code?: string }).code !== "engine.invalidSpec" && <Banner tone="error">{t("volOps.failed", { message: (m.error as { message: string }).message })}</Banner>}
          <Button variant="primary" className="w-full justify-center" disabled={!preview || errors.length > 0 || m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Plus size={15} />}
            {t("volCreate.submit")}
          </Button>
        </aside>
      </div>
    </div>
  );
}
