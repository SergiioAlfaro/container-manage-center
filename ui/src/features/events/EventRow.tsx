// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { memo, useMemo, useState } from "react";
import { ChevronRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { useImagesNav } from "@/features/images/store";
import { useNetworksNav } from "@/features/networks/store";
import { useVolumesNav } from "@/features/volumes/store";
import type { DockerEvent } from "@/shared/ipc/bindings/DockerEvent";
import { cx } from "@/shared/ui/controls";
import { eventDate, eventResource, formatEventTime } from "./model";

const TONE: Record<string, string> = {
  container: "bg-beacon/10 text-beacon-strong",
  image: "bg-ok/10 text-ok",
  volume: "bg-warn/10 text-warn",
  network: "bg-run/10 text-run",
};

/** Abre el recurso del evento en su apartado (si es de un tipo con detalle). */
export function openResource(e: DockerEvent): boolean {
  const name = eventResource(e);
  const app = useApp.getState();
  switch (e.kind) {
    case "container":
      useContainersNav.getState().showDetail(e.actorId, name);
      app.navigate("containers");
      return true;
    case "image":
      useImagesNav.getState().showDetail(e.actorId, e.name ?? name);
      app.navigate("images");
      return true;
    case "network":
      useNetworksNav.getState().showDetail(e.actorId, name);
      app.navigate("networks");
      return true;
    case "volume":
      useVolumesNav.getState().showDetail(e.actorId);
      app.navigate("volumes");
      return true;
    default:
      return false;
  }
}

const LINKABLE = ["container", "image", "network", "volume"];
const GONE = ["destroy", "delete", "remove", "untag", "prune"];

/** Memorizada: con cada evento nuevo solo se pinta su fila (Fase 9). */
export const EventRow = memo(function EventRow({ e, withDate }: { e: DockerEvent; withDate?: boolean }) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const res = eventResource(e);
  const attrs = Object.entries(e.attributes).filter(([k]) => k !== "name");
  const linkable = LINKABLE.includes(e.kind) && !GONE.includes(e.action.split(":")[0]!);
  return (
    <li className="border-b border-line last:border-0">
      <div className="flex items-center gap-3 px-4 py-1.5 text-[12.5px]">
        <button type="button" aria-expanded={open} aria-label={t("events.details")} onClick={() => setOpen(!open)} className="text-ink-3 hover:text-ink">
          <ChevronRight size={14} className={cx("transition-transform", open && "rotate-90")} />
        </button>
        <time className="w-[92px] shrink-0 font-mono text-[11.5px] text-ink-3 tabular-nums data-[date=true]:w-[178px]" data-date={withDate ? "true" : "false"} dateTime={eventDate(e).toISOString()}>
          {formatEventTime(e, i18n.language, withDate)}
        </time>
        <span className={cx("inline-flex h-5 w-[76px] shrink-0 items-center justify-center rounded text-[11px] font-medium", TONE[e.kind] ?? "bg-surface-sunk text-ink-2")}>
          {t(`events.type.${e.kind}`, { defaultValue: e.kind })}
        </span>
        <span className="w-[150px] shrink-0 truncate font-mono text-[12px]" title={e.action}>
          {e.action}
        </span>
        {linkable ? (
          <button type="button" onClick={() => openResource(e)} className="min-w-0 truncate text-left font-mono text-[12px] hover:text-beacon-strong hover:underline" title={e.actorId}>
            {res}
          </button>
        ) : (
          <span className="min-w-0 truncate font-mono text-[12px]" title={e.actorId}>
            {res}
          </span>
        )}
        {e.attributes.image && e.kind === "container" && <span className="ml-auto hidden truncate font-mono text-[11.5px] text-ink-3 lg:inline">{e.attributes.image}</span>}
      </div>
      {open && (
        <dl className="selectable mx-4 mb-2 grid grid-cols-[minmax(120px,max-content)_1fr] gap-x-4 gap-y-0.5 rounded-md bg-surface-2 px-3 py-2 font-mono text-[11.5px]">
          <dt className="text-ink-3">ID</dt>
          <dd className="[overflow-wrap:anywhere]">{e.actorId}</dd>
          {e.scope && (
            <>
              <dt className="text-ink-3">scope</dt>
              <dd>{e.scope}</dd>
            </>
          )}
          {attrs.map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-ink-3 [overflow-wrap:anywhere]">{k}</dt>
              <dd className="[overflow-wrap:anywhere]">{v}</dd>
            </div>
          ))}
        </dl>
      )}
    </li>
  );
});

/**
 * Clave estable de cada evento. No puede depender de la posición: los nuevos se
 * añaden arriba y, con la posición en la clave, React volvía a crear todas las
 * filas con cada evento (y se cerraba el detalle desplegado). Dos eventos
 * idénticos en el mismo nanosegundo se distinguen por orden de aparición.
 */
export function eventKeys(events: DockerEvent[]): string[] {
  const seen = new Map<string, number>();
  return events.map((e) => {
    const base = `${e.timeNano}|${e.kind}|${e.action}|${e.actorId}`;
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    return n === 0 ? base : `${base}|${n}`;
  });
}

export function EventList({ events, withDate, empty }: { events: DockerEvent[]; withDate?: boolean; empty: string }) {
  const keys = useMemo(() => eventKeys(events), [events]);
  if (events.length === 0) return <p className="rounded-xl border border-line bg-surface px-4 py-10 text-center text-[13px] text-ink-2">{empty}</p>;
  return (
    <ul data-testid="event-list" className="overflow-hidden rounded-xl border border-line bg-surface">
      {events.map((e, i) => (
        <EventRow key={keys[i]} e={e} withDate={withDate} />
      ))}
    </ul>
  );
}
