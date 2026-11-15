// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect } from "react";
import type { QueryKey } from "@tanstack/react-query";
import { LOCAL_SERVER, queryClientFor } from "@/app/server";
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
 *  pedir a Docker los datos afectados (actualización en tiempo real, D-057).
 *  0.10.0: cada evento lleva su servidor y solo se recarga la caché de ese
 *  servidor. */
export function useLiveRefresh() {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const pending = new Map<string, { server: string; key: QueryKey }>();
    let timer: number | undefined;
    let lastFlush = -Infinity;
    const flush = () => {
      timer = undefined;
      lastFlush = Date.now();
      for (const p of pending.values()) void queryClientFor(p.server).invalidateQueries({ queryKey: p.key });
      pending.clear();
    };
    void api
      .onDockerEvent((e) => {
        useEvents.getState().add(e);
        const server = e.server?.id ?? LOCAL_SERVER;
        for (const k of invalidationsFor(e)) pending.set(JSON.stringify([server, k]), { server, key: k });
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
  }, []);
}
