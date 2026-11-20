// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Registros construidos a mano SOLO para pruebas automatizadas.
 * Nunca se importan desde código de la aplicación.
 */
import type { OperationRecord } from "@/shared/ipc/bindings/OperationRecord";

let n = 0;

export function apiRecord(over: Partial<OperationRecord> = {}): OperationRecord {
  n += 1;
  return {
    id: `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`,
    category: "container",
    kind: "container.start",
    target: "web",
    transport: { via: "api", method: "POST", path: "/v1.47/containers/web/start", endpoint: "/var/run/docker.sock" },
    requestBody: null,
    equivalentCommand: "docker start web",
    status: "succeeded",
    startedAt: "2026-11-04T10:00:00.000Z",
    finishedAt: "2026-11-04T10:00:00.120Z",
    durationMs: 120,
    summary: null,
    stdout: null,
    stderr: null,
    error: null,
    httpStatus: 204,
    exitCode: null,
    server: null,
    batch: null,
    ...over,
  };
}

export function cliRecord(over: Partial<OperationRecord> = {}): OperationRecord {
  return apiRecord({
    category: "compose",
    kind: "compose.up",
    target: "blog",
    transport: { via: "cli", program: "docker", args: ["compose", "-f", "/srv/blog/compose.yaml", "up", "-d"] },
    equivalentCommand: null,
    httpStatus: null,
    exitCode: 0,
    stdout: { text: "Container blog-db-1 Started", truncated: false, originalBytes: 27 },
    ...over,
  });
}
