import { create } from "zustand";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { api, dockerApi } from "@/shared/ipc/client";
import type { ContainerAfter } from "@/shared/ipc/bindings/ContainerAfter";
import type { ShellChoice } from "@/shared/ipc/bindings/ShellChoice";
import type { TerminalEvent } from "@/shared/ipc/bindings/TerminalEvent";
import { decodeBase64 } from "@/features/containers/detail/TerminalPanel";
import { terminalTheme, trackTerminal } from "@/shared/ui/terminalTheme";

/**
 * Sesiones del apartado «Terminales» (0.9.1, D-064).
 *
 * Cada sesión es un `docker exec -it` real e independiente (su propio flujo en
 * el backend, su shell, su tamaño y su estado). El estado visible vive en un
 * store de Zustand; xterm y el identificador del flujo viven aquí, fuera de
 * React, para que cambiar de apartado no cierre ni vacíe las terminales: el
 * panel solo «presta» su elemento a la terminal mientras está montado.
 *
 * No hay sesiones huérfanas: cerrar una sesión cierra su flujo (también si aún
 * se estaba abriendo), y al cerrar la ventana el backend termina todas.
 */

export type SessionStatus = "opening" | "open" | "ended";

export interface Session {
  key: number;
  /** Servidor del contenedor (0.10.0): la sesión siempre va a ese servidor. */
  server: string;
  /** Nombre del servidor al abrirla (para la etiqueta; vacío = equipo local). */
  serverName: string;
  /** Servidor remoto. */
  remote: boolean;
  containerId: string;
  containerName: string;
  shell: ShellChoice;
  name: string;
  status: SessionStatus;
  /** Resumen con el que terminó (el mismo del panel de actividad). */
  summary: string | null;
  /** Error al abrir o durante la sesión. */
  error: string | null;
  /** El contenedor se detuvo o se eliminó y por eso terminó la sesión. */
  after: ContainerAfter | null;
}

/** Límite de sesiones simultáneas: cada una mantiene una shell en un contenedor. */
export const MAX_SESSIONS = 12;
export const MAX_NAME = 60;
export const SPLIT_MIN = 0.2;
export const SPLIT_MAX = 0.8;

interface Runtime {
  term: Terminal;
  fit: FitAddon;
  host: HTMLDivElement;
  opened: boolean;
  streamId: number | null;
  /** Se pidió cerrar mientras se abría: el flujo se cierra en cuanto llega su ID. */
  closeRequested: boolean;
  /** Cada apertura (o reapertura) es una generación; los eventos de una anterior se ignoran. */
  generation: number;
  dispose: () => void;
}

const runtimes = new Map<number, Runtime>();
let nextKey = 0;

interface State {
  sessions: Session[];
  active: number | null;
  maximized: number | null;
  split: { col: number; row: number };
}

export const useTerminals = create<State>(() => ({
  sessions: [],
  active: null,
  maximized: null,
  split: { col: 0.5, row: 0.5 },
}));

function patch(key: number, p: Partial<Session>) {
  useTerminals.setState((s) => ({ sessions: s.sessions.map((x) => (x.key === key ? { ...x, ...p } : x)) }));
}

const clampSplit = (v: number) => Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, Number.isFinite(v) ? v : 0.5));

/** Valida el nombre de una sesión; devuelve la clave del error o `null`. */
export function nameProblem(name: string): "empty" | "long" | "control" | null {
  const n = name.trim();
  if (n === "") return "empty";
  if ([...n].length > MAX_NAME) return "long";
  if (/[\u0000-\u001f\u007f]/.test(n)) return "control";
  return null;
}

/** Nombre por omisión: el del contenedor, numerado si ya hay sesiones en él. */
export function defaultName(containerName: string, sessions: Session[] = useTerminals.getState().sessions): string {
  const taken = new Set(sessions.map((s) => s.name));
  if (!taken.has(containerName)) return containerName;
  for (let i = 2; ; i++) {
    const n = `${containerName} (${i})`;
    if (!taken.has(n)) return n;
  }
}

function start(key: number) {
  const rt = runtimes.get(key);
  const s = useTerminals.getState().sessions.find((x) => x.key === key);
  if (!rt || !s) return;
  const gen = ++rt.generation;
  rt.streamId = null;
  rt.closeRequested = false;
  rt.term.reset();
  patch(key, { status: "opening", summary: null, error: null, after: null });
  let ended = false;
  const current = () => runtimes.get(key) === rt && rt.generation === gen;

  dockerApi(s.server)
    .terminalOpen(s.containerId, s.containerName, s.shell, rt.term.cols, rt.term.rows, (e: TerminalEvent) => {
      if (!current()) return;
      if (e.event === "output") {
        rt.term.write(decodeBase64(e.data));
        return;
      }
      ended = true;
      rt.streamId = null;
      const end = e.closed.end;
      const summary = end?.summary ?? null;
      const error = e.closed.error?.message ?? null;
      patch(key, { status: "ended", summary, error, after: end?.container ?? null });
      rt.term.write(`\r\n\x1b[2m— ${summary ?? error ?? ""} —\x1b[0m\r\n`);
    })
    .then((id) => {
      // Cerrada (o reabierta) mientras se abría: no se deja el flujo huérfano.
      if (!current() || rt.closeRequested) {
        void api.streamClose(id).catch(() => {});
        return;
      }
      if (ended) return;
      rt.streamId = id;
      patch(key, { status: "open" });
      // El tamaño pudo cambiar entre la petición y ahora.
      void api.terminalResize(id, rt.term.cols, rt.term.rows).catch(() => {});
      if (useTerminals.getState().active === key) rt.term.focus();
    })
    .catch((e: { message?: string }) => {
      if (!current()) return;
      patch(key, { status: "ended", error: e?.message ?? String(e) });
    });
}

/** Abre una sesión nueva en un contenedor. Devuelve su clave, o `null` si se alcanzó el límite. */
export function openSession(o: {
  server: string;
  serverName: string;
  remote: boolean;
  containerId: string;
  containerName: string;
  shell: ShellChoice;
  name?: string;
}): number | null {
  const st = useTerminals.getState();
  if (st.sessions.length >= MAX_SESSIONS) return null;
  const key = ++nextKey;
  const name = o.name && nameProblem(o.name) === null ? o.name.trim() : defaultName(o.containerName, st.sessions);
  const term = new Terminal({ fontFamily: "JetBrains Mono, monospace", fontSize: 13, cursorBlink: true, theme: terminalTheme(), scrollback: 5000, convertEol: false });
  const untrack = trackTerminal(term);
  const fit = new FitAddon();
  term.loadAddon(fit);
  const host = document.createElement("div");
  host.className = "h-full w-full";
  const rt: Runtime = { term, fit, host, opened: false, streamId: null, closeRequested: false, generation: 0, dispose: () => {} };
  const d1 = term.onData((data) => {
    if (rt.streamId !== null) void api.terminalInput(rt.streamId, data).catch(() => {});
  });
  const d2 = term.onResize(({ cols, rows }) => {
    if (rt.streamId !== null) void api.terminalResize(rt.streamId, cols, rows).catch(() => {});
  });
  rt.dispose = () => {
    d1.dispose();
    d2.dispose();
    untrack();
    term.dispose();
    host.remove();
  };
  runtimes.set(key, rt);
  useTerminals.setState((s) => ({
    sessions: [...s.sessions, { key, server: o.server, serverName: o.serverName, remote: o.remote, containerId: o.containerId, containerName: o.containerName, shell: o.shell, name, status: "opening", summary: null, error: null, after: null }],
    active: key,
    // Una sesión nueva se ve: si había otra maximizada, se vuelve a la cuadrícula.
    maximized: null,
  }));
  start(key);
  return key;
}

/** Vuelve a abrir una sesión terminada en el mismo contenedor y con la misma shell. */
export function reopenSession(key: number) {
  const s = useTerminals.getState().sessions.find((x) => x.key === key);
  if (!s || s.status !== "ended" || s.after === "removed") return;
  start(key);
}

/** Cierra la sesión (termina su shell en el contenedor) y quita su panel. */
export function closeSession(key: number) {
  const rt = runtimes.get(key);
  if (rt) {
    if (rt.streamId !== null) void api.streamClose(rt.streamId).catch(() => {});
    else rt.closeRequested = true;
    runtimes.delete(key);
    rt.dispose();
  }
  useTerminals.setState((s) => {
    const sessions = s.sessions.filter((x) => x.key !== key);
    const idx = s.sessions.findIndex((x) => x.key === key);
    const active = s.active === key ? (sessions[Math.min(idx, sessions.length - 1)]?.key ?? null) : s.active;
    return { sessions, active, maximized: s.maximized === key ? null : s.maximized };
  });
}

export function closeAllSessions() {
  for (const s of useTerminals.getState().sessions) closeSession(s.key);
}

export function renameSession(key: number, name: string): boolean {
  if (nameProblem(name) !== null) return false;
  patch(key, { name: name.trim() });
  return true;
}

export function setActive(key: number) {
  if (useTerminals.getState().active !== key) useTerminals.setState({ active: key });
}

export function toggleMaximize(key: number) {
  useTerminals.setState((s) => ({ maximized: s.maximized === key ? null : key, active: key }));
}

export function setSplit(p: Partial<State["split"]>) {
  useTerminals.setState((s) => ({
    split: { col: p.col !== undefined ? clampSplit(p.col) : s.split.col, row: p.row !== undefined ? clampSplit(p.row) : s.split.row },
  }));
}

/** Coloca la terminal de la sesión dentro de `el`; devuelve cómo retirarla. */
export function attachSession(key: number, el: HTMLElement): () => void {
  const rt = runtimes.get(key);
  if (!rt) return () => {};
  el.appendChild(rt.host);
  if (!rt.opened) {
    rt.term.open(rt.host);
    rt.opened = true;
  }
  fitSession(key);
  return () => {
    if (rt.host.parentElement === el) el.removeChild(rt.host);
  };
}

export function fitSession(key: number) {
  const rt = runtimes.get(key);
  if (!rt?.opened) return;
  try {
    rt.fit.fit();
  } catch {
    /* sin tamaño todavía (oculto) */
  }
}

export function focusSession(key: number) {
  runtimes.get(key)?.term.focus();
}

/** Solo para pruebas: cierra todo y vuelve al estado inicial. */
export function resetTerminals() {
  closeAllSessions();
  nextKey = 0;
  useTerminals.setState({ sessions: [], active: null, maximized: null, split: { col: 0.5, row: 0.5 } });
}
