import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "@/shared/ipc/client";
import type { StreamClosed } from "@/shared/ipc/bindings/StreamClosed";

export type StreamStatus = "idle" | "opening" | "open" | "ended";

type Ev = { event: string } & ({ event: "end"; closed: StreamClosed } | { event: string });

/**
 * Abre un flujo del backend mientras `open` no sea `null` y lo cierra al
 * desmontar o al cambiar `key`. Cada apertura es una operación real que queda
 * en el panel de actividad; cerrar la pestaña o la vista la cierra allí también.
 */
export function useStream<E extends Ev>(open: ((handler: (e: E) => void) => Promise<number>) | null, onEvent: (e: E) => void, key: string) {
  const [status, setStatus] = useState<StreamStatus>("idle");
  const [closed, setClosed] = useState<StreamClosed | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const handler = useRef(onEvent);
  handler.current = onEvent;
  const sid = useRef<number | null>(null);
  const opener = useRef(open);
  opener.current = open;
  const enabled = open !== null;

  useEffect(() => {
    const fn = opener.current;
    if (!enabled || !fn) {
      setStatus("idle");
      return;
    }
    let disposed = false;
    let ended = false;
    setStatus("opening");
    setClosed(null);
    setOpenError(null);
    fn((e) => {
      if (disposed) return;
      if (e.event === "end") {
        ended = true;
        sid.current = null;
        setClosed((e as { closed: StreamClosed }).closed);
        setStatus("ended");
      }
      handler.current(e);
    })
      .then((id) => {
        if (disposed) {
          void api.streamClose(id).catch(() => {});
          return;
        }
        if (!ended) {
          sid.current = id;
          setStatus("open");
        }
      })
      .catch((e) => {
        if (disposed) return;
        setOpenError((e as { message: string }).message);
        setStatus("ended");
      });
    return () => {
      disposed = true;
      const id = sid.current;
      sid.current = null;
      if (id !== null) void api.streamClose(id).catch(() => {});
    };
  }, [enabled, key]);

  const close = useCallback(() => {
    const id = sid.current;
    if (id !== null) void api.streamClose(id).catch(() => {});
  }, []);

  return { status, closed, openError, close, streamId: sid };
}
