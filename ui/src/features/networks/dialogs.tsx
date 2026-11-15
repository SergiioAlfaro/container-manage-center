// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useMemo, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link2, Trash2, Unlink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { shortId } from "@/shared/format";
import type { NetworkMember } from "@/shared/ipc/bindings/NetworkMember";
import type { NetworkSummary } from "@/shared/ipc/bindings/NetworkSummary";
import { keys } from "@/shared/query";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";
import { Field, Select, TextInput } from "@/features/containers/create/fields";
import { activeMembers, aliasesAllowed, connectBlock, connectCommand, disconnectCommand, pruneCandidates, splitAliases, stoppedMembers } from "./model";

type Net = Pick<NetworkSummary, "id" | "name" | "driver" | "predefined" | "containers" | "subnets">;

const errMsg = (e: unknown) => (e as { message: string }).message;

/** Recarga lo afectado. Tras eliminar no se recarga el detalle: la red ya no existe. */
export function useInvalidateNetworks(details = true) {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: keys.networks });
    void qc.invalidateQueries({ queryKey: keys.containers });
    void qc.invalidateQueries({ queryKey: keys.overview });
    if (details) void qc.invalidateQueries({ queryKey: ["network"] });
  };
}

/**
 * Lista de redes consultada a Docker al abrir el diálogo: los avisos y la
 * estimación no se basan en una lista que puede estar desactualizada (otro
 * programa o el CLI pueden haber cambiado las redes).
 */
function useFreshNetworks() {
  const api = useApi();
  const q = useQuery({ queryKey: keys.networks, queryFn: api.networksList, refetchOnMount: "always" });
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

function MemberList({ members }: { members: NetworkMember[] }) {
  const { t } = useTranslation();
  return (
    <ul className="mt-1 list-disc pl-5 font-mono text-[12px]">
      {members.map((m) => (
        <li key={m.id}>
          {m.name} <span className="font-sans text-ink-3">({t(`containers.state.${m.state}`, { defaultValue: m.state })})</span>
        </li>
      ))}
    </ul>
  );
}

/**
 * Eliminar una red. Docker rechaza la eliminación si hay contenedores en
 * marcha conectados, pero la permite si solo la tienen configurada contenedores
 * detenidos, que después no podrán arrancar: se avisa de ambos casos (D-047).
 */
export function RemoveNetworkDialog({ network: initial, onClose, onDone }: { network: Net; onClose: () => void; onDone: () => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const invalidate = useInvalidateNetworks(false);
  const { list, fresh } = useFreshNetworks();
  const current = list?.find((n) => n.id === initial.id);
  const network: Net = current ?? initial;
  const gone = fresh && !current;
  const running = activeMembers(network);
  const stopped = stoppedMembers(network);
  const [sure, setSure] = useState(false);
  const m = useMutation({
    mutationFn: () => api.networkRemove(network.id, network.name),
    onSuccess: () => {
      invalidate();
      onDone();
      onClose();
    },
  });
  const blocked = !fresh || gone || network.predefined || running.length > 0 || (stopped.length > 0 && !sure) || m.isPending;
  return (
    <Dialog
      title={t("netOps.removeTitle", { name: network.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={blocked} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Trash2 size={14} />}
            {t("netOps.remove")}
          </Button>
        </>
      }
    >
      <p className="selectable font-mono text-[12px] text-ink-3">
        {shortId(network.id)} · {network.driver}
      </p>
      {!fresh && <Checking />}
      {gone && <Banner tone="info">{t("netOps.gone", { name: network.name })}</Banner>}
      {network.predefined ? (
        <Banner tone="info">{t("netOps.predefined")}</Banner>
      ) : (
        <p className="text-[13px]">{t("netOps.removeBody")}</p>
      )}
      {fresh && running.length > 0 && (
        <Banner tone="error">
          <p>{t("netOps.runningBlocks", { count: running.length })}</p>
          <MemberList members={running} />
        </Banner>
      )}
      {fresh && stopped.length > 0 && (
        <Banner tone="warn">
          <p>{t("netOps.stoppedBreak", { count: stopped.length })}</p>
          <MemberList members={stopped} />
        </Banner>
      )}
      {fresh && stopped.length > 0 && running.length === 0 && !network.predefined && <Checkbox checked={sure} onChange={setSure} label={t("netOps.stoppedSure")} />}
      <p className="text-[12px] text-ink-3">
        {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">docker network rm {network.name}</code>
      </p>
      {m.error && <Banner tone="error">{t("netOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}

/** Limpieza: estima con la lista actual qué eliminará Docker y avisa de los contenedores detenidos afectados. */
export function PruneNetworksDialog({ onClose, onDone }: { onClose: () => void; onDone: (deleted: string[]) => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const invalidate = useInvalidateNetworks(false);
  const { list, fresh, error } = useFreshNetworks();
  const affected = fresh && list ? pruneCandidates(list) : [];
  const withStopped = affected.filter((n) => n.containers.length > 0);
  const [sure, setSure] = useState(false);
  const m = useMutation({
    mutationFn: api.networksPrune,
    onSuccess: (r) => {
      invalidate();
      onDone(r);
      onClose();
    },
  });
  return (
    <Dialog
      title={t("netOps.pruneTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={!fresh || (withStopped.length > 0 && !sure) || m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Trash2 size={14} />}
            {t("netOps.prune")}
          </Button>
        </>
      }
    >
      <p className="text-[13px]">{t("netOps.pruneBody")}</p>
      {error && <Banner tone="error">{t("networks.loadError", { message: errMsg(error) })}</Banner>}
      {fresh ? <p className="text-[13px]">{t("netOps.pruneEstimate", { count: affected.length })}</p> : !error && <Checking />}
      {affected.length > 0 && (
        <ul className="max-h-[160px] overflow-auto rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-[12px]">
          {affected.map((n) => (
            <li key={n.id}>
              {n.name}
              {n.containers.length > 0 && <span className="font-sans text-warn"> — {t("netOps.usedByStopped", { names: n.containers.map((c) => c.name).join(", ") })}</span>}
            </li>
          ))}
        </ul>
      )}
      {withStopped.length > 0 && (
        <>
          <Banner tone="warn">{t("netOps.pruneStoppedWarn", { count: withStopped.length })}</Banner>
          <Checkbox checked={sure} onChange={setSure} label={t("netOps.stoppedSure")} />
        </>
      )}
      <p className="text-[12px] text-ink-3">
        {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">docker network prune --force</code>
      </p>
      {m.error && <Banner tone="error">{t("netOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}

/** Conectar un contenedor a la red, con alias e IP fija opcionales. */
export function ConnectDialog({ network: initial, initialContainer, onClose, onDone }: { network: Net; initialContainer?: string; onClose: () => void; onDone: (msg: string) => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const invalidate = useInvalidateNetworks();
  const { list, fresh: netFresh } = useFreshNetworks();
  const network: Net = list?.find((n) => n.id === initial.id) ?? initial;
  const containers = useQuery({ queryKey: keys.containers, queryFn: api.containersList, refetchOnMount: "always" });
  const options = useMemo(
    () => (containers.data ?? []).map((c) => ({ c, block: connectBlock(network, c) })).sort((a, b) => Number(a.block !== null) - Number(b.block !== null) || a.c.name.localeCompare(b.c.name)),
    [containers.data, network],
  );
  const firstFree = options.find((o) => o.block === null)?.c.id ?? "";
  const [picked, setPicked] = useState<string | null>(initialContainer ?? null);
  const target = picked ?? firstFree;
  const chosen = options.find((o) => o.c.id === target);
  const [aliases, setAliases] = useState("");
  const [ipv4, setIpv4] = useState("");
  const [ipv6, setIpv6] = useState("");
  const allowAliases = aliasesAllowed(network);
  const opts = { aliases: allowAliases ? splitAliases(aliases) : [], ipv4, ipv6 };
  const m = useMutation({
    mutationFn: () => api.networkConnect(network.id, network.name, target, chosen?.c.name ?? null, opts, network.subnets),
    onSuccess: () => {
      invalidate();
      onDone(t("netOps.connected", { container: chosen?.c.name ?? target, network: network.name }));
      onClose();
    },
  });
  const fresh = netFresh && containers.isFetchedAfterMount && !containers.isFetching;
  const ok = fresh && !!chosen && chosen.block === null && !m.isPending;
  return (
    <Dialog
      title={t("netOps.connectTitle", { name: network.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="primary" disabled={!ok} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Link2 size={14} />}
            {t("netOps.connect")}
          </Button>
        </>
      }
    >
      {!fresh ? (
        <Checking />
      ) : options.length === 0 ? (
        <p className="text-[13px] text-ink-3">{t("netOps.noContainers")}</p>
      ) : (
        <Field label={t("netOps.container")} hint={chosen?.block ? t(`netOps.block.${chosen.block}`) : t("netOps.containerHint")}>
          {(id) => (
            <Select id={id} value={target} onChange={(e) => setPicked(e.target.value)}>
              {options.map(({ c, block }) => (
                <option key={c.id} value={c.id} disabled={block !== null}>
                  {c.name} — {t(`containers.state.${c.state}`, { defaultValue: c.state })}
                  {block ? ` (${t(`netOps.blockShort.${block}`)})` : ""}
                </option>
              ))}
            </Select>
          )}
        </Field>
      )}
      {allowAliases ? (
        <Field label={t("netOps.aliases")} hint={t("netOps.aliasesHint")}>
          {(id) => <TextInput id={id} value={aliases} onChange={(e) => setAliases(e.target.value)} placeholder="web api" />}
        </Field>
      ) : (
        <p className="text-[12px] text-ink-3">{t("netOps.noAliases")}</p>
      )}
      <div className="grid gap-3 md:grid-cols-2">
        <Field label={t("netOps.ipv4")} hint={network.subnets.length ? t("netOps.ipHint", { subnets: network.subnets.join(", ") }) : t("netOps.ipAuto")}>
          {(id) => <TextInput id={id} value={ipv4} onChange={(e) => setIpv4(e.target.value)} placeholder={t("netOps.auto")} />}
        </Field>
        <Field label={t("netOps.ipv6")}>{(id) => <TextInput id={id} value={ipv6} onChange={(e) => setIpv6(e.target.value)} placeholder={t("netOps.auto")} />}</Field>
      </div>
      {chosen && (
        <p className="text-[12px] text-ink-3">
          {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">{connectCommand(network.name, chosen.c.name, opts)}</code>
        </p>
      )}
      {m.error && <Banner tone="error">{t("netOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}

/** Desconectar: corta la conectividad del contenedor por esta red. */
export function DisconnectDialog({
  network,
  member,
  onClose,
  onDone,
  lastNetwork = false,
}: {
  network: Pick<NetworkSummary, "id" | "name">;
  member: NetworkMember;
  onClose: () => void;
  onDone: (msg: string) => void;
  /** Es la única red del contenedor (0.9.2): se queda sin conectividad. */
  lastNetwork?: boolean;
}) {
  const api = useApi();
  const { t } = useTranslation();
  const invalidate = useInvalidateNetworks();
  const [force, setForce] = useState(false);
  const m = useMutation({
    mutationFn: () => api.networkDisconnect(network.id, network.name, member.id, member.name, force),
    onSuccess: () => {
      invalidate();
      onDone(t("netOps.disconnected", { container: member.name, network: network.name }));
      onClose();
    },
  });
  return (
    <Dialog
      title={t("netOps.disconnectTitle", { container: member.name, network: network.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Unlink size={14} />}
            {t("netOps.disconnect")}
          </Button>
        </>
      }
    >
      <p className="text-[13px]">{member.state === "running" ? t("netOps.disconnectRunning") : t("netOps.disconnectStopped")}</p>
      {lastNetwork && <Banner tone="warn">{t("netOps.lastNetwork")}</Banner>}
      <Checkbox checked={force} onChange={setForce} label={t("netOps.force")} hint={t("netOps.forceHint")} />
      <p className="text-[12px] text-ink-3">
        {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">{disconnectCommand(network.name, member.name, force)}</code>
      </p>
      {m.error && <Banner tone="error">{t("netOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}
