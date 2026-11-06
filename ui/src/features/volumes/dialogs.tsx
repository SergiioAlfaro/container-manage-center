import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useEngine } from "@/features/engine/store";
import { formatBytes } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { VolumePruneResult } from "@/shared/ipc/bindings/VolumePruneResult";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";
import type { VolumeUser } from "@/shared/ipc/bindings/VolumeUser";
import { keys } from "@/shared/query";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";
import { legacyPrune, pruneCandidates } from "./model";

const errMsg = (e: unknown) => (e as { message: string }).message;

export function useApiVersion(): string | null {
  return useEngine((s) => (s.state?.state === "connected" ? s.state.engine.apiVersion : null));
}

/** Recarga lo afectado. Tras eliminar no se recarga el detalle: el volumen ya no existe. */
export function useInvalidateVolumes(details = true) {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: keys.volumes });
    void qc.invalidateQueries({ queryKey: keys.volumeSizes });
    void qc.invalidateQueries({ queryKey: keys.overview });
    if (details) void qc.invalidateQueries({ queryKey: ["volume"] });
  };
}

/** Lista consultada a Docker al abrir el diálogo (no una copia desactualizada, D-049). */
function useFreshVolumes() {
  const q = useQuery({ queryKey: keys.volumes, queryFn: api.volumesList, refetchOnMount: "always" });
  return { list: q.data, fresh: q.isFetchedAfterMount && !q.isFetching, error: q.error };
}

function Checking() {
  const { t } = useTranslation();
  return (
    <p className="flex items-center gap-2 text-[13px] text-ink-3">
      <Spinner size={14} /> {t("netOps.checking")}
    </p>
  );
}

function UserList({ users }: { users: VolumeUser[] }) {
  const { t } = useTranslation();
  return (
    <ul className="mt-1 list-disc pl-5 font-mono text-[12px]">
      {users.map((u) => (
        <li key={`${u.id}-${u.destination}`}>
          {u.name} <span className="font-sans text-ink-3">({t(`containers.state.${u.state}`, { defaultValue: u.state })}, {u.destination})</span>
        </li>
      ))}
    </ul>
  );
}

/**
 * Eliminar un volumen. Docker lo rechaza si lo monta cualquier contenedor, en
 * marcha o detenido, también forzando (D-051): en ese caso no se deja continuar.
 */
export function RemoveVolumeDialog({ volume: initial, onClose, onDone }: { volume: Pick<VolumeSummary, "name" | "containers" | "anonymous">; onClose: () => void; onDone: () => void }) {
  const { t } = useTranslation();
  const invalidate = useInvalidateVolumes(false);
  const { list, fresh } = useFreshVolumes();
  const current = list?.find((v) => v.name === initial.name);
  const v = current ?? initial;
  const gone = fresh && !current;
  const [sure, setSure] = useState(false);
  const m = useMutation({
    mutationFn: () => api.volumeRemove(v.name),
    onSuccess: () => {
      invalidate();
      onDone();
      onClose();
    },
  });
  const inUse = v.containers.length > 0;
  const blocked = !fresh || gone || inUse || !sure || m.isPending;
  return (
    <Dialog
      title={t("volOps.removeTitle", { name: v.anonymous ? v.name.slice(0, 12) : v.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={blocked} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Trash2 size={14} />}
            {t("volOps.remove")}
          </Button>
        </>
      }
    >
      {!fresh && <Checking />}
      {gone && <Banner tone="info">{t("volOps.gone", { name: v.name })}</Banner>}
      <p className="text-[13px]">{t("volOps.removeBody")}</p>
      {fresh && inUse && (
        <Banner tone="error">
          <p>{t("volOps.inUse", { count: v.containers.length })}</p>
          <UserList users={v.containers} />
        </Banner>
      )}
      {fresh && !inUse && !gone && <Checkbox checked={sure} onChange={setSure} label={t("volOps.sure")} />}
      <p className="text-[12px] text-ink-3">
        {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">docker volume rm {v.name}</code>
      </p>
      {m.error && <Banner tone="error">{t("volOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}

/** Limpieza: anónimos sin uso o, con confirmación adicional, todos los volúmenes sin uso. */
export function PruneVolumesDialog({ onClose, onDone }: { onClose: () => void; onDone: (r: VolumePruneResult) => void }) {
  const { t, i18n } = useTranslation();
  const invalidate = useInvalidateVolumes(false);
  const apiVersion = useApiVersion();
  const legacy = legacyPrune(apiVersion);
  const { list, fresh, error } = useFreshVolumes();
  const sizes = useQuery({ queryKey: keys.volumeSizes, queryFn: api.volumeSizes, enabled: false });
  const [all, setAll] = useState(false);
  const [sure, setSure] = useState(false);
  const affected = fresh && list ? pruneCandidates(list, all, apiVersion) : [];
  const named = affected.filter((v) => !v.anonymous);
  const known = sizes.data ? affected.map((v) => sizes.data.find((s) => s.name === v.name)?.size ?? null) : [];
  const bytes = known.length && known.every((b) => b !== null) ? known.reduce<number>((a, b) => a + (b ?? 0), 0) : null;
  const m = useMutation({
    mutationFn: () => api.volumesPrune(all && !legacy),
    onSuccess: (r) => {
      invalidate();
      onDone(r);
      onClose();
    },
  });
  const needSure = named.length > 0;
  return (
    <Dialog
      title={t("volOps.pruneTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={!fresh || (needSure && !sure) || m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Trash2 size={14} />}
            {t("volOps.prune")}
          </Button>
        </>
      }
    >
      {legacy ? (
        <Banner tone="warn">{t("volOps.legacy", { api: apiVersion })}</Banner>
      ) : (
        <fieldset className="space-y-2">
          <label className="flex items-start gap-2 text-[13px]">
            <input type="radio" name="vprune" className="mt-1" checked={!all} onChange={() => setAll(false)} />
            <span>
              {t("volOps.pruneAnonymous")}
              <span className="block text-[12px] text-ink-3">docker volume prune --force</span>
            </span>
          </label>
          <label className="flex items-start gap-2 text-[13px]">
            <input type="radio" name="vprune" className="mt-1" checked={all} onChange={() => setAll(true)} />
            <span>
              {t("volOps.pruneAll")}
              <span className="block text-[12px] text-ink-3">docker volume prune --force --all</span>
            </span>
          </label>
        </fieldset>
      )}
      {error && <Banner tone="error">{t("volumes.loadError", { message: errMsg(error) })}</Banner>}
      {fresh ? (
        <p className="text-[13px]">
          {t("volOps.pruneEstimate", { count: affected.length })}
          {bytes !== null && affected.length > 0 && ` ${t("volOps.pruneBytes", { size: formatBytes(bytes, i18n.language) })}`}
          {affected.length > 0 ? ":" : "."}
        </p>
      ) : (
        !error && <Checking />
      )}
      {affected.length > 0 && (
        <ul className="max-h-[160px] overflow-auto rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-[12px]">
          {affected.map((v) => (
            <li key={v.name}>
              {v.anonymous ? `${v.name.slice(0, 12)} (${t("volumes.anonymous")})` : v.name}
            </li>
          ))}
        </ul>
      )}
      <p className="text-[12px] text-ink-3">{t("volOps.pruneNote")}</p>
      {needSure && <Checkbox checked={sure} onChange={setSure} label={t("volOps.pruneSure", { count: named.length })} />}
      {m.error && <Banner tone="error">{t("volOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}
