/**
 * Editores de listas del asistente de creación (variables, puertos, montajes y
 * memoria). Se comparten con «Editar configuración» (0.9.2) para que las dos
 * pantallas se comporten igual.
 */
import { useEffect, useState } from "react";
import { useIsRemote } from "@/features/servers/paths";
import { Eye, EyeOff, FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/ipc/client";
import type { KeyValue } from "@/shared/ipc/bindings/KeyValue";
import type { MountSpec } from "@/shared/ipc/bindings/MountSpec";
import type { PortSpec } from "@/shared/ipc/bindings/PortSpec";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import { Button, Segmented } from "@/shared/ui/controls";
import { Checkbox } from "@/shared/ui/feedback";
import { isSecretKey, issuesFor, type MemUnit } from "../specModel";
import { Field, IssueList, RowList, Select, TextInput } from "./fields";

export const port = (s: string): number | null => {
  const v = s.trim();
  if (v === "") return null;
  const n = Number(v);
  // Fuera de rango → 0, que el backend señala como puerto no válido.
  return Number.isInteger(n) && n > 0 && n <= 65535 ? n : 0;
};

export function PortsEditor({ ports, onChange, issues }: { ports: PortSpec[]; onChange: (v: PortSpec[]) => void; issues: SpecIssue[] }) {
  const { t } = useTranslation();
  const of = (f: string) => issuesFor(issues, f);
  return (
    <RowList<PortSpec>
      rows={ports}
      onChange={onChange}
      make={() => ({ hostIp: "", hostPort: null, containerPort: 0, protocol: "tcp" })}
      addLabel={t("create.add.port")}
      removeLabel={(i) => t("create.remove.port", { n: i + 1 })}
      empty={t("create.empty.ports")}
      render={(p, setP, i) => (
        <div className="grid grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)_minmax(0,1fr)_90px] gap-2">
          <Field label={t("create.f.hostIp")} issues={of(`ports.${i}.hostIp`)}>
            {(id, inv) => <TextInput id={id} invalid={inv} value={p.hostIp} onChange={(e) => setP({ ...p, hostIp: e.target.value })} placeholder={t("create.ph.allIps")} />}
          </Field>
          <Field label={t("create.f.hostPort")} issues={of(`ports.${i}.hostPort`)}>
            {(id, inv) => <PortInput id={id} invalid={inv} value={p.hostPort} onChange={(v) => setP({ ...p, hostPort: v })} placeholder={t("create.ph.anyPort")} />}
          </Field>
          <Field label={t("create.f.containerPort")} issues={of(`ports.${i}.containerPort`)}>
            {(id, inv) => <PortInput id={id} invalid={inv} value={p.containerPort || null} onChange={(v) => setP({ ...p, containerPort: v ?? 0 })} placeholder="80" />}
          </Field>
          <Field label={t("create.f.protocol")}>
            {(id) => (
              <Select id={id} value={p.protocol} onChange={(e) => setP({ ...p, protocol: e.target.value as PortSpec["protocol"] })}>
                <option value="tcp">tcp</option>
                <option value="udp">udp</option>
                <option value="sctp">sctp</option>
              </Select>
            )}
          </Field>
        </div>
      )}
    />
  );
}

/** Volúmenes y carpetas. Las sugerencias de volúmenes vienen de un `<datalist id="dcc-volumes">`. */
export function MountsEditor({ mounts, onChange, issues }: { mounts: MountSpec[]; onChange: (v: MountSpec[]) => void; issues: SpecIssue[] }) {
  // En un servidor remoto la carpeta es del servidor: no hay selector local (0.10.0).
  const remote = useIsRemote();
  const { t } = useTranslation();
  const of = (f: string) => issuesFor(issues, f);
  return (
    <RowList<MountSpec>
      rows={mounts}
      onChange={onChange}
      make={() => ({ kind: "volume", source: "", target: "", readOnly: false })}
      addLabel={t("create.add.mount")}
      removeLabel={(i) => t("create.remove.mount", { n: i + 1 })}
      empty={t("create.empty.mounts")}
      render={(m, setM, i) => (
        <div className="space-y-2 rounded-lg border border-line p-3">
          <div className="flex flex-wrap items-center gap-3">
            <Segmented<MountSpec["kind"]>
              label={t("create.f.mountKind")}
              value={m.kind}
              onChange={(k) => setM({ ...m, kind: k, source: "" })}
              options={[
                { value: "volume", label: t("create.mountKind.volume") },
                { value: "bind", label: t("create.mountKind.bind") },
              ]}
            />
            <Checkbox checked={m.readOnly} onChange={(v) => setM({ ...m, readOnly: v })} label={t("create.f.readOnly")} />
          </div>
          <div className="grid gap-2 md:grid-cols-2">
            <Field
              label={m.kind === "volume" ? t("create.f.volumeName") : remote ? t("servers.paths.bindLabel") : t("create.f.hostPath")}
              issues={of(`mounts.${i}.source`)}
              hint={m.kind === "volume" ? t("create.h.volumeName") : remote ? t("servers.paths.bindHint") : undefined}
            >
              {(id, inv) =>
                m.kind === "volume" ? (
                  <TextInput id={id} invalid={inv} list="dcc-volumes" value={m.source} onChange={(e) => setM({ ...m, source: e.target.value })} placeholder={t("create.ph.anonymous")} />
                ) : (
                  <div className="flex gap-1.5">
                    <TextInput id={id} invalid={inv} value={m.source} onChange={(e) => setM({ ...m, source: e.target.value })} placeholder={remote ? "/srv/datos" : "/home/usuario/datos"} />
                    {!remote && <Button
                      size="sm"
                      className="h-8 shrink-0"
                      aria-label={t("create.pickFolder")}
                      title={t("create.pickFolder")}
                      onClick={async () => {
                        const dir = await api.pickFolder(t("create.pickFolder"));
                        if (dir) setM({ ...m, source: dir });
                      }}
                    >
                      <FolderOpen size={14} />
                    </Button>}
                  </div>
                )
              }
            </Field>
            <Field label={t("create.f.target")} issues={of(`mounts.${i}.target`)}>
              {(id, inv) => <TextInput id={id} invalid={inv} value={m.target} onChange={(e) => setM({ ...m, target: e.target.value })} placeholder="/data" />}
            </Field>
          </div>
        </div>
      )}
    />
  );
}

export function PortInput({ value, onChange, ...rest }: { value: number | null; onChange: (v: number | null) => void; id: string; invalid: boolean; placeholder: string }) {
  const [text, setText] = useState(value === null ? "" : String(value));
  useEffect(() => {
    if (port(text) !== value && !(value === 0 && port(text) === 0)) setText(value === null ? "" : String(value));
    // Solo sincroniza cuando el valor cambia desde fuera (p. ej. al añadir los puertos expuestos).
  }, [value]);
  return (
    <TextInput
      {...rest}
      inputMode="numeric"
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        onChange(port(e.target.value));
      }}
    />
  );
}

export function MemField({ label, value, onChange, issues }: { label: string; value: { value: string; unit: MemUnit }; onChange: (v: { value: string; unit: MemUnit }) => void; issues: SpecIssue[] }) {
  const { t } = useTranslation();
  return (
    <Field label={label} issues={issues}>
      {(id, inv) => (
        <div className="flex gap-1.5">
          <TextInput id={id} invalid={inv} inputMode="decimal" value={value.value} onChange={(e) => onChange({ ...value, value: e.target.value })} placeholder={t("create.ph.unlimited")} />
          <div className="w-[80px] shrink-0">
            <Select aria-label={t("create.f.unit")} value={value.unit} onChange={(e) => onChange({ ...value, unit: e.target.value as MemUnit })}>
              <option value="m">MiB</option>
              <option value="g">GiB</option>
            </Select>
          </div>
        </div>
      )}
    </Field>
  );
}

export function KvEditor({ rows, onChange, field, issues, keyPh, valuePh, secret = false, addLabel, empty }: { rows: KeyValue[]; onChange: (v: KeyValue[]) => void; field: string; issues: SpecIssue[]; keyPh: string; valuePh: string; secret?: boolean; addLabel: string; empty: string }) {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState<Record<number, boolean>>({});
  return (
    <RowList<KeyValue>
      rows={rows}
      onChange={onChange}
      make={() => ({ key: "", value: "" })}
      addLabel={addLabel}
      removeLabel={(i) => t("create.remove.row", { n: i + 1 })}
      empty={empty}
      render={(kv, setKv, i) => {
        const hidden = secret && isSecretKey(kv.key) && !revealed[i];
        const own = [...issuesFor(issues, `${field}.${i}.key`), ...issuesFor(issues, `${field}.${i}.value`)];
        return (
          <div>
            <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)] gap-2">
              <TextInput aria-label={t("create.f.key", { n: i + 1 })} invalid={issuesFor(issues, `${field}.${i}.key`).some((x) => x.severity === "error")} value={kv.key} onChange={(e) => setKv({ ...kv, key: e.target.value })} placeholder={keyPh} />
              <div className="flex gap-1">
                <TextInput
                  aria-label={t("create.f.value", { n: i + 1 })}
                  invalid={issuesFor(issues, `${field}.${i}.value`).some((x) => x.severity === "error")}
                  type={hidden ? "password" : "text"}
                  value={kv.value}
                  onChange={(e) => setKv({ ...kv, value: e.target.value })}
                  placeholder={valuePh}
                />
                {secret && isSecretKey(kv.key) && (
                  <button
                    type="button"
                    onClick={() => setRevealed((r) => ({ ...r, [i]: !r[i] }))}
                    aria-label={hidden ? t("detail.reveal") : t("detail.hide")}
                    title={hidden ? t("detail.reveal") : t("detail.hide")}
                    className="grid h-8 w-8 shrink-0 place-items-center rounded-md text-ink-3 hover:bg-surface-sunk hover:text-ink"
                  >
                    {hidden ? <Eye size={15} /> : <EyeOff size={15} />}
                  </button>
                )}
              </div>
            </div>
            <IssueList issues={own} />
          </div>
        );
      }}
    />
  );
}

