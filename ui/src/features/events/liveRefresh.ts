import { useEffect } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { api } from "@/shared/ipc/client";
import { invalidationsFor } from "./model";
import { useEvents } from "./store";

/** Espera antes de recargar: un `docker compose up` produce decenas de eventos seguidos. */
export const REFRESH_DELAY_MS = 250;
/** Separación mínima entre recargas mientras siguen llegando eventos (Fase 9):
 *  en una ráfaga larga (cien `docker start` seguidos desde la terminal) la
 *  interfaz se actualiza una vez por segundo en lugar de con cada evento. */
export const REFRESH_MIN_INTERVAL_MS = 1000;

/** Cuánto esperar para recargar, sabiendo cuándo fue la última recarga. */
export function refreshDelay(now: number, lastFlush: number): number {
  return Math.max(REFRESH_DELAY_MS, lastFlush + REFRESH_MIN_INTERVAL_MS - now);
}

/** Recibe los eventos de Docker: los añade a la vista en directo y vuelve a
 *  pedir a Docker los datos afectados (actualización en tiempo real, D-057). */
export function useLiveRefresh() {
  const qc = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const pending = new Map<string, QueryKey>();
    let timer: number | undefined;
    let lastFlush = -Infinity;
    const flush = () => {
      timer = undefined;
      lastFlush = Date.now();
      for (const k of pending.values()) void qc.invalidateQueries({ queryKey: k });
      pending.clear();
    };
    void api
      .onDockerEvent((e) => {
        useEvents.getState().add(e);
        for (const k of invalidationsFor(e)) pending.set(JSON.stringify(k), k);
        if (pending.size > 0 && timer === undefined) timer = window.setTimeout(flush, refreshDelay(Date.now(), lastFlush));
      })
      .then((un) => {
        if (disposed) un();
        else unlisten = un;
      });
    return () => {
      disposed = true;
      unlisten?.();
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [qc]);
}
