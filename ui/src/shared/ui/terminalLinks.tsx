// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useState } from "react";
import { ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { create } from "zustand";
import type { ILinkHandler } from "@xterm/xterm";
import { api } from "@/shared/ipc/client";
import { Button } from "@/shared/ui/controls";
import { Banner, Dialog } from "@/shared/ui/feedback";

/**
 * Enlaces de la salida de las terminales (OSC 8) (E2, CMC-SEC-12, D-091).
 *
 * xterm, sin manejador propio, abriría el enlace con `window.open` tras un
 * `confirm()` del navegador. Aquí: solo `http://` y `https://`, siempre con
 * una confirmación de la aplicación que muestra la dirección completa, y se
 * abre en el navegador del sistema desde Rust (que vuelve a validarla).
 */
export const MAX_LINK = 2048;

export function linkAllowed(uri: string): boolean {
  // eslint-disable-next-line no-control-regex -- se buscan caracteres de control a propósito
  if (uri.length > MAX_LINK || !/^https?:\/\//.test(uri) || /[\s\u0000-\u001f\u007f]/.test(uri)) return false;
  try {
    const u = new URL(uri);
    return (u.protocol === "http:" || u.protocol === "https:") && u.hostname !== "" && u.username === "" && u.password === "";
  } catch {
    return false;
  }
}

export const useLinkPrompt = create<{ url: string | null; ask: (url: string) => void; close: () => void }>((set) => ({
  url: null,
  ask: (url) => set({ url }),
  close: () => set({ url: null }),
}));

/** Opción `linkHandler` para cada `Terminal`. */
export const terminalLinkHandler: ILinkHandler = {
  activate: (_event, uri) => {
    if (linkAllowed(uri)) useLinkPrompt.getState().ask(uri);
  },
  allowNonHttpProtocols: false,
};

/** Confirmación (una en toda la aplicación). */
export function TerminalLinkPrompt() {
  const { t } = useTranslation();
  const url = useLinkPrompt((s) => s.url);
  const close = useLinkPrompt((s) => s.close);
  const [error, setError] = useState<string | null>(null);
  if (!url) return null;
  const done = () => {
    setError(null);
    close();
  };
  return (
    <Dialog
      target={false}
      title={t("terminalLink.title")}
      onClose={done}
      footer={
        <>
          <Button onClick={done}>{t("common.cancel")}</Button>
          <Button
            variant="primary"
            onClick={() => {
              api
                .openTerminalLink(url)
                .then(done)
                .catch((e: { message: string }) => setError(e.message));
            }}
          >
            <ExternalLink size={14} /> {t("terminalLink.open")}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <p className="text-[13px] text-ink-2">{t("terminalLink.body")}</p>
        <code className="selectable block break-all rounded-md bg-surface-2 px-3 py-2 font-mono text-[12.5px] text-ink">{url}</code>
        {error && <Banner tone="error">{error}</Banner>}
      </div>
    </Dialog>
  );
}
