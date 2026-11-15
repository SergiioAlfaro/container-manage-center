// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Servidor remoto sin conexión en el área principal: nombre, dirección,
 * estado, causa y solución, y la acción para conectar. Ninguna operación se
 * hace sin conexión, y nunca se usa otro servidor en su lugar.
 */
import { Plug, Server, Stethoscope } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useServers, useServerState, useServerView } from "@/app/server";
import { Button } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { isIdentityProblem, IdentityReview } from "./IdentityReview";
import { useServersNav } from "./nav";
import { displayAddress, remoteProblemOf, statusOf } from "./remote";
import { RemoteProblemView, StatusLabel } from "./ui";

export function RemotePanel() {
  const { t } = useTranslation();
  const view = useServerView();
  const state = useServerState();
  const busy = useServers((s) => (view ? !!s.busy[view.id] : false));
  const connect = useServers((s) => s.connect);
  const navigate = useApp((s) => s.navigate);
  const nav = useServersNav();
  if (!view?.config) return null;
  const c = view.config;
  const problem = remoteProblemOf(state);
  const go = (f: () => void) => {
    f();
    navigate("servers");
  };
  return (
    <div className="mx-auto max-w-[860px] space-y-4 px-8 py-8" data-testid="remote-panel">
      <section className="rounded-xl border border-line bg-surface px-6 py-5">
        <div className="flex flex-wrap items-start gap-4">
          <div className="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-beacon-soft text-beacon-strong">
            <Server size={20} strokeWidth={1.75} />
          </div>
          <div className="min-w-[220px] flex-1">
            <h2 className="text-[17px] font-semibold tracking-[-0.01em]">{c.name}</h2>
            <p className="selectable font-mono text-[12.5px] text-ink-2">{displayAddress(c)}</p>
            <div className="mt-1.5 text-[13px]">
              {busy ? (
                <span className="inline-flex items-center gap-1.5">
                  <Spinner size={12} /> {t("servers.status.connecting")}
                </span>
              ) : (
                <StatusLabel status={statusOf(state)} />
              )}
            </div>
          </div>
          <div className="flex shrink-0 flex-wrap gap-2">
            <Button onClick={() => go(() => nav.diagnose(view.id))}>
              <Stethoscope size={14} />
              {t("servers.diagnose")}
            </Button>
            <Button onClick={() => go(() => nav.edit(view.id))}>{t("servers.edit")}</Button>
            <Button variant="primary" disabled={busy} onClick={() => void connect(view.id)}>
              {busy ? <Spinner size={14} /> : <Plug size={14} />}
              {t("servers.connect")}
            </Button>
          </div>
        </div>
        {!problem && <p className="mt-4 text-[13px] text-ink-2">{t("servers.notConnectedBody")}</p>}
      </section>
      {problem &&
        (isIdentityProblem(problem) ? (
          <IdentityReview config={c} problem={problem} onTrusted={() => void connect(view.id)} />
        ) : (
          <RemoteProblemView problem={problem} config={c} />
        ))}
    </div>
  );
}
