import { useEffect } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { api } from "@/shared/ipc/client";
import { invalidationsFor } from "./model";
import { useEvents } from "./store";

/** Espera antes de recargar: un `docker compose up` produce decenas de eventos seguidos. */
export const REFRESH_DELAY_MS = 250;

/** Recibe los eventos de Docker: los añade a la vista en directo y vuelve a
 *  pedir a Docker los datos afectados (actualización en tiempo real, D-057). */
export function useLiveRefresh() {
  const qc = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const pending = new Map<string, QueryKey>();
    let timer: number | undefined;
    const flush = () => {
      timer = undefined;
      for (const k of pending.values()) void qc.invalidateQueries({ queryKey: k });
      pending.clear();
    };
    void api
      .onDockerEvent((e) => {
        useEvents.getState().add(e);
        for (const k of invalidationsFor(e)) pending.set(JSON.stringify(k), k);
        if (pending.size > 0 && timer === undefined) timer = window.setTimeout(flush, REFRESH_DELAY_MS);
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
