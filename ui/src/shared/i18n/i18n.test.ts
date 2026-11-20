// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import es from "./locales/es.json";
import en from "./locales/en.json";
import fr from "./locales/fr.json";
import it_ from "./locales/it.json";
import ca from "./locales/ca.json";
import gl from "./locales/gl.json";
import eu from "./locales/eu.json";
import pt from "./locales/pt.json";
import i18n, { LANGUAGES, LANGUAGE_NAMES } from "./index";

function keys(obj: unknown, prefix = ""): string[] {
  if (obj === null || typeof obj !== "object" || Array.isArray(obj)) return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) => keys(v, prefix ? `${prefix}.${k}` : k));
}
const get = (o: unknown, k: string) => k.split(".").reduce<unknown>((x, p) => (x as Record<string, unknown>)?.[p], o);
const ALL = { es, en, ca, gl, eu, fr, it: it_, pt } as const;

/**
 * Textos que pueden coincidir con el inglés porque son iguales en ese idioma:
 * nombres propios, términos técnicos que no se traducen y palabras que se
 * escriben igual. Cualquier otra cadena igual al inglés se considera sin
 * traducir. La lista es por valor y por idioma, para que cada excepción sea
 * explícita.
 */
const TECHNICAL = [
  "Container Manage Center", "CMC", "API", "CLI", "Socket", "Compose", "Español", "English", "Français", "Italiano", "Docker {{version}}",
  "rootless", "Rootless", "{{message}}", "Docker Engine", "cgroups", "Go", "Commit", "{{action}} {{name}}", "{{action}} {{name}}: {{message}}",
  "ID", "PID", "TTY", "CPU", "Shell", "nginx, postgres, python…", "Dockerfile", "cache", "IPv4", "IPv6", "MAC", "tmpfs", "NFS", "Docker",
  "Config", "WebKitGTK", "glibc", "Docker CLI", "Docker Compose", "Debian 13:", "Compose: {{project}}",
  // 0.9.2: distintivo de operaciones de la propia aplicación y nombre de la API.
  "APP", "Docker Engine API", "Digests",
  // 0.10.0: columna de versión y recursos de un servidor (nombres de producto y unidades).
  "Docker / API", "{{cpus}} CPU · {{memory}}", "SSH (docker system dial-stdio)",
  // 1.3.0: auditoría (nombres de tecnologías y de formatos de archivo).
  "rootless", "userns-remap", "seccomp", "AppArmor", "SELinux", "cgroup", "JSON", "Markdown", "HTML", "Docker",
  "{{name}} {{version}}: {{reason}}",
  // 1.4.0: copias (nombres de los motores de bases de datos).
  "MariaDB", "MySQL", "PostgreSQL", "Redis", "MongoDB",
  // 1.5.0: monitorización (unidades de tiempo y de recursos).
  "5 min", "15 min", "30 min", "1 h", "6 h", "24 h", "{{cpus}} CPU · {{mem}}", "{{h}} h {{m}} min", "{{m}} min", "{{s}} s", "PIDs", "{{d}} d {{h}} h",
  // 1.7.0: nombre del apartado «Legal» (se escribe igual en es, ca, gl y pt).
  "Legal",
];
const SAME_WORDS: Record<"fr" | "it" | "ca" | "gl" | "eu" | "pt", string[]> = {
  fr: [
    "Orchestration", "Images", "Image", "Volumes", "Volume", "Ports", "Actions", "Options", "{{count}} image", "{{count}} images", "{{count}} volume",
    "{{count}} volumes", "Versions", "Version", "Terminal", "Description", "Digests", "Instruction", "Type", "Local", "Services", "Service",
    "Configuration", "Pause", "Secret", "Application", "Architecture", "Variables", "volume",
    // 1.3.0
    "Audit", "Date",
    // 1.7.0
    "Licence",
  ],
  it: [
    "Dashboard", "Area", "No", "Host", "Kernel", "{{count}} container", "Entrypoint", "Gateway", "Container {{name}}", "Container", "Volume",
    "{{count}} volume", "Repository", "Tag", "Driver", "Local", "File", "Output", "Plugin", "Computer", "Desktop", "volume", "REPOSITORY:TAG",
    // 0.10.0: en italiano se usan «server» y «password».
    "Server {{name}}", "Password", "Server",
    // 1.3.0
    "Audit", "no", "container {{name}}",
    // 1.4.0: en italiano también se dice «database».
    "Database",
    // 1.10.0: gestión masiva («container» no cambia en singular).
    "{{op}}: {{count}} container",
  ],
  ca: [
    // 1.3.0
    "no",
    "Context", "Equivalent", "Error", "Error: {{message}}", "Executable", "General", "Local", "No", "Node", "Ports", "Protocol", "Secret",
    "Terminal", "Terminals", "Variables", "Versions", "{{count}} error", "{{count}} errors",
    // 1.4.0
    "Manual",
    // 1.10.0
    "Total",
  ],
  gl: ["Daemon (dockerd)", "Executable", "Local", "Terminal", "Variables", "Volume", "Volumes", "volume", "{{count}} volume", "{{count}} volumes", "Manual", "Total"],
  eu: [],
  pt: ["Aliases", "Ctrl+Shift+A", "Daemon (dockerd)", "Gateway", "Local", "Terminal", "Volume", "Volumes", "volume", "{{count}} volume", "{{count}} volumes", "Manual", "Total"],
};

describe("traducciones", () => {
  it("los ocho idiomas están en el selector, en el orden pedido, y en i18n", () => {
    expect(LANGUAGES).toEqual(["en", "es", "ca", "gl", "eu", "fr", "it", "pt"]);
    expect(LANGUAGES.map((l) => LANGUAGE_NAMES[l])).toEqual(["English", "Español", "Català", "Galego", "Euskara", "Français", "Italiano", "Português"]);
    for (const l of LANGUAGES) expect(i18n.hasResourceBundle(l, "translation"), l).toBe(true);
  });

  it("todos los idiomas tienen exactamente las mismas claves", () => {
    const ref = keys(es).sort();
    for (const [l, d] of Object.entries(ALL)) expect(keys(d).sort(), l).toEqual(ref);
  });

  it("ninguna traducción está vacía", () => {
    for (const [l, d] of Object.entries(ALL)) {
      for (const k of keys(d)) {
        const v = get(d, k);
        if (typeof v === "string") expect(v.trim(), `${l}:${k}`).not.toBe("");
      }
    }
  });

  it("cada traducción conserva las mismas variables {{…}} que el español", () => {
    const vars = (s: unknown) => (typeof s === "string" ? [...s.matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).sort() : []);
    for (const k of keys(es)) {
      // Las formas de plural pueden omitir {{count}} ("Una sesión").
      const base = vars(get(es, k)).filter((v) => v !== "count");
      for (const [l, d] of Object.entries(ALL)) expect(vars(get(d, k)).filter((v) => v !== "count"), `${l}:${k}`).toEqual(base);
    }
  });

  it("ningún idioma deja textos sin traducir (iguales al inglés)", () => {
    for (const [l, d] of [["fr", fr], ["it", it_], ["ca", ca], ["gl", gl], ["eu", eu], ["pt", pt]] as const) {
      const allowed = new Set([...TECHNICAL, ...SAME_WORDS[l]]);
      const untranslated = keys(en).filter((k) => {
        const v = get(d, k);
        return typeof v === "string" && v === get(en, k) && !allowed.has(v);
      });
      expect(untranslated, l).toEqual([]);
    }
  });

  it("al cambiar de idioma, los textos de la interfaz cambian", async () => {
    // Textos distintos en los ocho idiomas.
    const distinct = ["sections.containers.title", "components.title", "help.github", "errorCodes.engine_notConnected", "edit.action", "commit.action"];
    // Textos que coinciden legítimamente entre lenguas cercanas («Cancelar» en es/gl/pt,
    // «Novo terminal» en gl/pt…): cambian, pero no son únicos por idioma.
    const shared = ["terminals.new", "common.cancel", "settings.themeOptions.dark"];
    const seen = new Map<string, Set<string>>();
    const fingerprints = new Set<string>();
    for (const l of LANGUAGES) {
      await i18n.changeLanguage(l);
      const all: string[] = [];
      for (const k of [...distinct, ...shared]) {
        const v = i18n.t(k);
        expect(v, `${l}:${k}`).not.toBe(k);
        expect(v, `${l}:${k}`).toBe(get(ALL[l], k));
        seen.set(k, (seen.get(k) ?? new Set()).add(v));
        all.push(v);
      }
      fingerprints.add(all.join("|"));
    }
    await i18n.changeLanguage("es");
    for (const k of distinct) expect(seen.get(k)!.size, k).toBe(LANGUAGES.length);
    for (const k of shared) expect(seen.get(k)!.size, k).toBeGreaterThanOrEqual(6);
    expect(fingerprints.size).toBe(LANGUAGES.length);
  });
});
