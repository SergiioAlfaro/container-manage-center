import { describe, expect, it } from "vitest";
import es from "./locales/es.json";
import en from "./locales/en.json";
import fr from "./locales/fr.json";
import it_ from "./locales/it.json";
import i18n, { LANGUAGES } from "./index";

function keys(obj: unknown, prefix = ""): string[] {
  if (obj === null || typeof obj !== "object" || Array.isArray(obj)) return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) => keys(v, prefix ? `${prefix}.${k}` : k));
}
const get = (o: unknown, k: string) => k.split(".").reduce<unknown>((x, p) => (x as Record<string, unknown>)?.[p], o);
const ALL = { es, en, fr, it: it_ } as const;

/**
 * Textos que pueden coincidir con el inglés porque son iguales en ese idioma:
 * nombres propios, términos técnicos que no se traducen y palabras que se
 * escriben igual. Cualquier otra cadena igual al inglés se considera sin
 * traducir. La lista es por valor y por idioma, para que cada excepción sea
 * explícita.
 */
const TECHNICAL = [
  "Docker Control Center", "API", "CLI", "Socket", "Compose", "Español", "English", "Français", "Italiano", "Docker {{version}}",
  "rootless", "Rootless", "{{message}}", "Docker Engine", "cgroups", "Go", "Commit", "{{action}} {{name}}", "{{action}} {{name}}: {{message}}",
  "ID", "PID", "TTY", "CPU", "Shell", "nginx, postgres, python…", "Dockerfile", "cache", "IPv4", "IPv6", "MAC", "tmpfs", "NFS", "Docker",
  "Config", "WebKitGTK", "glibc", "Docker CLI", "Docker Compose", "Debian 13:", "Compose: {{project}}",
];
const SAME_WORDS: Record<"fr" | "it", string[]> = {
  fr: [
    "Orchestration", "Images", "Image", "Volumes", "Volume", "Ports", "Actions", "Options", "{{count}} image", "{{count}} images", "{{count}} volume",
    "{{count}} volumes", "Versions", "Version", "Terminal", "Description", "Digests", "Instruction", "Type", "Local", "Services", "Service",
    "Configuration", "Pause", "Secret", "Application", "Architecture",
  ],
  it: [
    "Dashboard", "Area", "No", "Host", "Kernel", "{{count}} container", "Entrypoint", "Gateway", "Container {{name}}", "Container", "Volume",
    "{{count}} volume", "Repository", "Tag", "Driver", "Local", "File", "Output", "Plugin", "Computer", "Desktop",
  ],
};

describe("traducciones", () => {
  it("los cuatro idiomas están en el selector y en i18n", () => {
    expect(LANGUAGES).toEqual(["es", "en", "fr", "it"]);
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

  it("francés e italiano no dejan textos sin traducir (iguales al inglés)", () => {
    for (const [l, d] of [["fr", fr], ["it", it_]] as const) {
      const allowed = new Set([...TECHNICAL, ...SAME_WORDS[l]]);
      const untranslated = keys(en).filter((k) => {
        const v = get(d, k);
        return typeof v === "string" && v === get(en, k) && !allowed.has(v);
      });
      expect(untranslated, l).toEqual([]);
    }
  });

  it("al cambiar de idioma, los textos de la interfaz cambian", async () => {
    const sample = ["sections.containers.title", "terminals.new", "components.title", "help.github", "common.cancel", "errorCodes.engine_notConnected"];
    const seen = new Map<string, Set<string>>();
    for (const l of LANGUAGES) {
      await i18n.changeLanguage(l);
      for (const k of sample) {
        const v = i18n.t(k);
        expect(v, `${l}:${k}`).not.toBe(k);
        seen.set(k, (seen.get(k) ?? new Set()).add(v));
      }
    }
    await i18n.changeLanguage("es");
    for (const [k, s] of seen) expect(s.size, k).toBe(4);
  });
});
