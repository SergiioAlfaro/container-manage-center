// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { create } from "zustand";
import { api, dockerApi } from "@/shared/ipc/client";
import type { AuditProgress } from "@/shared/ipc/bindings/AuditProgress";
import type { AuditReport } from "@/shared/ipc/bindings/AuditReport";
import type { AuditRequest } from "@/shared/ipc/bindings/AuditRequest";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";

export type RunStatus = "idle" | "running" | "done" | "failed" | "cancelled";

export interface AuditRun {
  status: RunStatus;
  stream: number | null;
  progress: AuditProgress | null;
  /** Informe que se muestra (el último o uno del historial). */
  report: AuditReport | null;
  error: CommandError | null;
  cancelRequested: boolean;
}

const empty: AuditRun = { status: "idle", stream: null, progress: null, report: null, error: null, cancelRequested: false };

interface Audits {
  runs: Record<string, AuditRun>;
  start: (server: string, request: AuditRequest, onFinished?: (ok: boolean) => void) => Promise<void>;
  cancel: (server: string) => void;
  show: (server: string, report: AuditReport) => void;
}

/** Auditoría en curso y la que se muestra, por servidor: siguen al cambiar de
 *  apartado y nunca se mezclan entre servidores. */
export const useAudits = create<Audits>((set, get) => {
  const cur = (server: string) => get().runs[server] ?? empty;
  const put = (server: string, patch: Partial<AuditRun>) => set((s) => ({ runs: { ...s.runs, [server]: { ...(s.runs[server] ?? empty), ...patch } } }));
  return {
    runs: {},
    start: async (server, request, onFinished) => {
      if (cur(server).status === "running") return;
      put(server, { ...empty, status: "running", report: cur(server).report });
      try {
        const stream = await dockerApi(server).auditRunOpen(request, (e) => {
          if (e.event === "progress") {
            put(server, { progress: e.progress });
          } else {
            const cancelled = cur(server).cancelRequested;
            if (e.report) put(server, { status: "done", stream: null, report: e.report, error: null, progress: null });
            else put(server, { status: cancelled ? "cancelled" : "failed", stream: null, error: cancelled ? null : e.error, progress: null });
            onFinished?.(!!e.report);
          }
        });
        if (cur(server).status === "running") {
          put(server, { stream });
          if (cur(server).cancelRequested) void api.streamClose(stream);
        }
      } catch (e) {
        put(server, { status: "failed", error: e as CommandError });
        onFinished?.(false);
      }
    },
    cancel: (server) => {
      put(server, { cancelRequested: true });
      const id = cur(server).stream;
      if (id !== null) void api.streamClose(id);
    },
    show: (server, report) => {
      if (cur(server).status === "running") return;
      put(server, { report, status: "done", error: null });
    },
  };
});
