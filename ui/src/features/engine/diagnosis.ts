// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import type { Diagnosis } from "@/shared/ipc/bindings/Diagnosis";

/** Un paso de la solución: texto traducible y, opcionalmente, una orden para copiar. */
export interface Step {
  key: string;
  params?: Record<string, string>;
  command?: string;
}

export interface Explanation {
  /** Clave de `diag.problems.*`. */
  problem: string;
  params: Record<string, string>;
  steps: Step[];
  /** Mostrar el aviso de que el grupo docker equivale a root. */
  groupWarning?: string;
}

const isRootlessPath = (p: string) => /^\/run\/user\/\d+\//.test(p) || p.includes("/docker-rootless") || p.includes("/.docker/run/");

const startSteps = (path: string | null): Step[] => {
  const rootless = path !== null && isRootlessPath(path);
  return rootless
    ? [
        { key: "startRootless", command: "systemctl --user enable --now docker" },
        { key: "checkStatus", command: "systemctl --user status docker --no-pager" },
      ]
    : [
        { key: "startSystem", command: "sudo systemctl enable --now docker" },
        { key: "checkStatus", command: "sudo systemctl status docker --no-pager" },
      ];
};

/**
 * Traduce el diagnóstico del backend en una explicación con pasos concretos.
 * Nunca propone que la aplicación ejecute nada: son órdenes para el usuario.
 */
export function explain(d: Diagnosis): Explanation {
  const p = d.problem;
  const retry: Step = { key: "retry" };
  switch (p.kind) {
    case "noSocket": {
      const inst = d.installation;
      if (!inst.cliPath && !inst.daemonPath) {
        return {
          problem: "notInstalled",
          params: {},
          steps: [{ key: "install", command: "sudo apt install docker.io" }, { key: "installOfficial" }, ...startSteps(null), retry],
        };
      }
      const rootlessOnly = inst.rootlessTools && !inst.daemonPath;
      return { problem: "noSocket", params: {}, steps: [...startSteps(rootlessOnly ? `/run/user/${d.user.uid}/docker.sock` : null), retry] };
    }
    case "daemonNotRunning":
      return { problem: "daemonNotRunning", params: { path: p.socketPath }, steps: [...startSteps(p.socketPath), retry] };
    case "permissionDenied": {
      const s = p.permissions;
      const params = { user: d.user.name, path: s.socketPath, owner: s.owner, group: s.group, mode: s.mode };
      if (s.group !== "docker") {
        return { problem: "permissionDenied", params, steps: [{ key: "ownerUnexpected" }, { key: "rootlessAlternative" }, retry] };
      }
      if (s.userInGroup && !s.groupActiveInSession) {
        return { problem: "permissionDenied", params, steps: [{ key: "alreadyInGroup", params: { group: s.group } }, retry] };
      }
      return {
        problem: "permissionDenied",
        params,
        groupWarning: s.group,
        steps: [
          { key: "addGroup", params: { group: s.group }, command: `sudo usermod -aG ${s.group} ${d.user.name}` },
          { key: "relogin" },
          { key: "rootlessAlternative" },
          retry,
        ],
      };
    }
    case "unsupportedHost":
      return {
        problem: "unsupportedHost",
        params: { host: p.host },
        steps: [{ key: "unsetHost", command: "unset DOCKER_HOST; docker context use default" }, { key: "customPath" }, retry],
      };
    case "invalidPath":
      return { problem: "invalidPath", params: { path: p.path, reason: p.reason }, steps: [{ key: "fixPath" }] };
    case "notASocket":
      return { problem: "notASocket", params: { path: p.socketPath }, steps: [{ key: "checkPath" }, { key: "fixPath" }] };
    case "notDocker":
      return { problem: "notDocker", params: { path: p.socketPath, message: p.message }, steps: [{ key: "checkPath" }, { key: "fixPath" }] };
    case "timeout":
      return {
        problem: "timeout",
        params: { path: p.socketPath },
        steps: [
          isRootlessPath(p.socketPath)
            ? { key: "restart", command: "systemctl --user restart docker" }
            : { key: "restart", command: "sudo systemctl restart docker" },
          retry,
        ],
      };
    case "apiTooOld":
      return { problem: "apiTooOld", params: { api: p.apiVersion, min: p.minimum }, steps: [{ key: "upgrade" }, { key: "installOfficial" }, retry] };
    case "other":
      return { problem: "other", params: { message: p.message }, steps: [retry] };
    case "remote":
      // Servidores remotos: su diagnóstico propio está en la sección Servidores.
      return { problem: "other", params: { message: p.remote.kind }, steps: [retry] };
  }
}
