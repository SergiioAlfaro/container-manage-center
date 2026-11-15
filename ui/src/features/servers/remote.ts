// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Servidores remotos (0.10.0): explicación de cada problema de conexión y
 * pasos para resolverlo. Igual que el diagnóstico local, nunca propone que la
 * aplicación ejecute nada en el servidor: no instala Docker, no cambia
 * permisos, no toca la configuración SSH ni abre puertos. Son órdenes para que
 * las revise y ejecute el usuario, en el equipo indicado.
 */
import type { RemoteProblem } from "@/shared/ipc/bindings/RemoteProblem";
import type { ServerConfig } from "@/shared/ipc/bindings/ServerConfig";
import type { ServerView } from "@/shared/ipc/bindings/ServerView";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";
import type { CheckId } from "@/shared/ipc/bindings/CheckId";
import type { Step } from "@/features/engine/diagnosis";

/** Dónde se ejecuta una orden propuesta. */
export type Where = "local" | "server";

export interface RemoteStep extends Step {
  where?: Where;
}

export interface RemoteExplanation {
  /** Clave de `servers.problems.*`. */
  problem: string;
  params: Record<string, string>;
  steps: RemoteStep[];
  /** Paso de la prueba en el que falla. */
  check: CheckId | null;
  /** Aviso de seguridad destacado (identidad cambiada). */
  danger?: boolean;
  /** Mostrar el aviso de que el grupo docker equivale a root. */
  groupWarning?: boolean;
}

/** `[host]:puerto` o `host`, como en known_hosts. */
export function hostPattern(c: Pick<ServerConfig, "host" | "port">): string {
  return c.port === 22 ? c.host : `[${c.host}]:${c.port}`;
}

/** `usuario@host` o `usuario@host:puerto` para mostrar. */
export function displayAddress(c: Pick<ServerConfig, "host" | "port" | "user">): string {
  const host = c.host.includes(":") ? `[${c.host}]` : c.host;
  return c.port === 22 ? `${c.user}@${host}` : `${c.user}@${host}:${c.port}`;
}

/** Orden `ssh` para que el usuario pruebe por su cuenta (argumentos entrecomillados). */
export function sshCommand(c: Pick<ServerConfig, "host" | "port" | "user">, remote?: string): string {
  const q = (s: string) => (/^[\w@%+=:,./-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`);
  const parts = ["ssh", ...(c.port !== 22 ? ["-p", String(c.port)] : []), q(`${c.user}@${c.host}`)];
  if (remote) parts.push(q(remote));
  return parts.join(" ");
}

export function explainRemote(p: RemoteProblem, c: ServerConfig): RemoteExplanation {
  const retry: RemoteStep = { key: "retry" };
  const host = c.host;
  const port = String(c.port);
  switch (p.kind) {
    case "sshMissing":
      return {
        problem: "sshMissing",
        params: {},
        check: "reach",
        steps: [{ key: "installSsh", command: "sudo apt install openssh-client", where: "local" }, retry],
      };
    case "invalidProfile":
      return { problem: "invalidProfile", params: { codes: p.codes.join(", ") }, check: null, steps: [{ key: "editProfile" }] };
    case "hostUnresolvable":
      return {
        problem: "hostUnresolvable",
        params: { host: p.host },
        check: "resolve",
        steps: [{ key: "checkName" }, { key: "resolveName", command: `getent hosts ${p.host}`, where: "local" }, retry],
      };
    case "hostUnreachable":
      return {
        problem: "hostUnreachable",
        params: { host, message: p.message },
        check: "reach",
        steps: [{ key: "checkNetwork" }, { key: "ping", command: `ping -c 3 ${host}`, where: "local" }, retry],
      };
    case "sshRefused":
      return {
        problem: "sshRefused",
        params: { host, port: String(p.port) },
        check: "reach",
        steps: [{ key: "checkPort" }, { key: "sshdStatus", command: "sudo systemctl status ssh --no-pager", where: "server" }, retry],
      };
    case "notSsh":
      return { problem: "notSsh", params: { host, port }, check: "reach", steps: [{ key: "checkPort" }, retry] };
    case "timeout":
      return {
        problem: "timeout",
        params: { host, port, seconds: String(c.connectTimeout) },
        check: "reach",
        steps: [{ key: "checkFirewall" }, { key: "raiseTimeout" }, retry],
      };
    case "hostKeyUnknown":
      return {
        problem: "hostKeyUnknown",
        params: { host, fingerprint: p.presented?.fingerprint ?? "", type: p.presented?.keyType ?? "" },
        check: "identity",
        steps: [{ key: "verifyFingerprint", command: "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub", where: "server" }, { key: "trustIfMatches" }],
      };
    case "hostKeyChanged":
      return {
        problem: "hostKeyChanged",
        params: { host, fingerprint: p.presented?.fingerprint ?? "", type: p.presented?.keyType ?? "", file: p.file ?? "" },
        check: "identity",
        danger: true,
        steps: [
          { key: "doNotConnect" },
          { key: "askAdmin" },
          { key: "verifyFingerprint", command: "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub", where: "server" },
          p.inUserFile ? { key: "removeUserKey", command: `ssh-keygen -R '${hostPattern(c)}'`, where: "local" } : { key: "replaceInApp" },
        ],
      };
    // E1 (D-087): el servidor intermedio se verifica como el destino.
    case "jumpHostKeyUnknown":
      return {
        problem: "jumpHostKeyUnknown",
        params: { jump: p.jump, fingerprint: p.presented?.fingerprint ?? "", type: p.presented?.keyType ?? "" },
        check: "identity",
        steps: [{ key: "verifyJumpFingerprint", command: "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub", where: "server" }, { key: "trustIfMatches" }],
      };
    case "jumpHostKeyChanged":
      return {
        problem: "jumpHostKeyChanged",
        params: { jump: p.jump, fingerprint: p.presented?.fingerprint ?? "", type: p.presented?.keyType ?? "", file: p.file ?? "" },
        check: "identity",
        danger: true,
        steps: [{ key: "doNotConnect" }, { key: "askAdmin" }, { key: "verifyJumpFingerprint", command: "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub", where: "server" }, p.inUserFile ? { key: "removeUserKey", command: `ssh-keygen -R '${p.jump.replace(/^.*@/, "")}'`, where: "local" } : { key: "replaceInApp" }],
      };
    case "jumpAuthFailed":
      return {
        problem: "jumpAuthFailed",
        params: { jump: p.jump, methods: p.methods ?? "" },
        check: "auth",
        steps: [{ key: "jumpUseKey" }, { key: "agentKeys", command: "ssh-add -l", where: "local" }, { key: "tryJump", command: `ssh -o BatchMode=yes ${p.jump.includes(":") && !p.jump.includes("[") ? `-p ${p.jump.split(":").pop()} ${p.jump.replace(/:\d+$/, "")}` : p.jump} true`, where: "local" }, retry],
      };
    case "jumpFromSshConfig":
      return { problem: "jumpFromSshConfig", params: { via: p.via, host }, check: "reach", steps: [{ key: "jumpInProfile" }, { key: "editProfile" }] };
    case "authFailed":
      return {
        problem: c.auth === "password" ? "authFailedPassword" : c.auth === "agent" ? "authFailedAgent" : "authFailedKey",
        params: { user: c.user, host, methods: p.methods ?? "" },
        check: "auth",
        steps:
          c.auth === "password"
            ? [{ key: "checkPassword" }, { key: "passwordDisabled" }, retry]
            : c.auth === "agent"
              ? [
                  { key: "agentKeys", command: "ssh-add -l", where: "local" },
                  { key: "authorizedKeys" },
                  { key: "tryYourself", command: sshCommand(c, "docker version"), where: "local" },
                  retry,
                ]
              : [
                  { key: "checkKey", params: { path: c.keyPath } },
                  { key: "authorizedKeys" },
                  { key: "tryYourself", command: sshCommand(c, "docker version"), where: "local" },
                  retry,
                ],
      };
    case "secretRequired":
      return { problem: p.secret === "password" ? "passwordRequired" : "passphraseRequired", params: {}, check: "auth", steps: [{ key: "enterSecret" }] };
    case "keyUnreadable":
      return {
        problem: "keyUnreadable",
        params: { path: p.path },
        check: "auth",
        steps: [{ key: "checkKeyPath", params: { path: p.path } }, { key: "keyPermissions", command: `chmod 600 ${p.path}`, where: "local" }, retry],
      };
    case "dockerMissing":
      return {
        problem: "dockerMissing",
        params: { host },
        check: "docker",
        steps: [{ key: "installDocker" }, { key: "dockerOnPath", command: "command -v docker", where: "server" }, retry],
      };
    case "dockerNotRunning":
      return {
        problem: "dockerNotRunning",
        params: { host, message: p.message },
        check: "permission",
        steps: [
          { key: "startDocker", command: "sudo systemctl enable --now docker", where: "server" },
          { key: "dockerStatus", command: "sudo systemctl status docker --no-pager", where: "server" },
          retry,
        ],
      };
    case "socketDenied":
      return {
        problem: "socketDenied",
        params: { user: c.user, host, message: p.message },
        check: "permission",
        groupWarning: true,
        steps: [
          { key: "askAdminAccess" },
          { key: "addToGroup", command: `sudo usermod -aG docker ${c.user}`, where: "server" },
          { key: "relogin" },
          { key: "rootlessOption" },
          retry,
        ],
      };
    case "contextMissing":
      return {
        problem: "contextMissing",
        params: { name: p.name, host },
        check: "docker",
        steps: [{ key: "listContexts", command: "docker context ls", where: "server" }, { key: "editProfile" }],
      };
    case "notDocker":
      return { problem: "notDocker", params: { host, message: p.message }, check: "permission", steps: [{ key: "checkSocket" }, { key: "editProfile" }] };
    case "apiTooOld":
      return { problem: "apiTooOld", params: { version: p.apiVersion, minimum: p.minimum, host }, check: "api", steps: [{ key: "upgradeDocker" }] };
    case "connectionLost":
      return { problem: "connectionLost", params: { host }, check: null, steps: [retry] };
    case "other":
      return { problem: "other", params: { message: p.message }, check: null, steps: [retry] };
  }
}

/** Problema remoto de un estado de conexión, si lo hay. */
export function remoteProblemOf(s: ConnectionState | null | undefined): RemoteProblem | null {
  if (s?.state !== "disconnected") return null;
  return s.diagnosis.problem.kind === "remote" ? s.diagnosis.problem.remote : null;
}

/** Estado resumido para la lista (§3: indicadores de estado). */
export type ServerStatus = "connected" | "disconnected" | "connecting" | "authError" | "connectionError" | "dockerUnavailable" | "identityError";

export function statusOf(s: ConnectionState | null | undefined): ServerStatus {
  if (!s || s.state === "idle") return "disconnected";
  if (s.state === "connecting") return "connecting";
  if (s.state === "connected") return "connected";
  const p = s.diagnosis.problem.kind === "remote" ? s.diagnosis.problem.remote : null;
  if (!p) return "dockerUnavailable";
  switch (p.kind) {
    case "authFailed":
    case "secretRequired":
    case "keyUnreadable":
      return "authError";
    case "hostKeyUnknown":
    case "hostKeyChanged":
      return "identityError";
    case "dockerMissing":
    case "dockerNotRunning":
    case "socketDenied":
    case "contextMissing":
    case "notDocker":
    case "apiTooOld":
      return "dockerUnavailable";
    default:
      return "connectionError";
  }
}

/** Color del indicador de estado. */
export function statusTone(s: ServerStatus): "ok" | "err" | "warn" | "idle" | "busy" {
  switch (s) {
    case "connected":
      return "ok";
    case "connecting":
      return "busy";
    case "disconnected":
      return "idle";
    case "dockerUnavailable":
      return "warn";
    default:
      return "err";
  }
}

/** Perfil vacío para el asistente (puerto 22, agente SSH, Docker por defecto). */
export function emptyServer(): ServerConfig {
  return {
    id: "",
    name: "",
    host: "",
    port: 22,
    user: "",
    description: "",
    auth: "agent",
    keyPath: "",
    dockerTarget: "default",
    dockerSocket: "",
    dockerContext: "",
    connectTimeout: 10,
    proxyJump: "",
    autoConnect: false,
    rememberSecret: false,
    lastSeen: null,
    composeProjects: [],
  };
}

/** Un servidor remoto de la lista con su perfil. */
export function remotes(list: ServerView[]): (ServerView & { config: ServerConfig })[] {
  return list.filter((s): s is ServerView & { config: ServerConfig } => s.remote && s.config !== null);
}
