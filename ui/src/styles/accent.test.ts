import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Color principal (0.9.2, D-072): los contrastes se calculan con la fórmula de
 * WCAG 2.x a partir de los tokens reales de app.css, para los dos colores y
 * los dos temas. Umbrales: 4.5:1 para texto y 3:1 para elementos de interfaz
 * (bordes, indicadores, iconos y anillo de foco).
 */
const css = readFileSync(resolve(__dirname, "app.css"), "utf8");

function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const body = css.slice(start, css.indexOf("}", start));
  return Object.fromEntries([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]));
}

type Rgba = [number, number, number, number];
function parse(v: string): Rgba {
  const hex = /^#([0-9a-f]{6})$/i.exec(v);
  if (hex) return [0, 2, 4].map((i) => parseInt(hex[1]!.slice(i, i + 2), 16)).concat(1) as Rgba;
  const rgb = /^rgb\((\d+) (\d+) (\d+) \/ ([\d.]+)\)$/.exec(v);
  if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3]), Number(rgb[4])];
  throw new Error(`color no reconocido: ${v}`);
}
/** Color con transparencia compuesto sobre un fondo opaco. */
const over = (fg: Rgba, bg: Rgba): Rgba => [0, 1, 2].map((i) => fg[i]! * fg[3] + bg[i]! * (1 - fg[3])).concat(1) as Rgba;
const lum = ([r, g, b]: Rgba) => {
  const f = (c: number) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};
const contrast = (a: Rgba, b: Rgba) => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
};

const light = block(":root");
const dark = { ...light, ...block(':root[data-theme="dark"]') };
const THEMES = {
  "verde azulado · claro": light,
  "verde azulado · oscuro": dark,
  "morado · claro": { ...light, ...block(':root[data-accent="purple"]') },
  "morado · oscuro": { ...dark, ...block(':root[data-accent="purple"]'), ...block(':root[data-theme="dark"][data-accent="purple"]') },
};

describe("color principal: contraste WCAG", () => {
  it.each(Object.entries(THEMES))("%s", (_name, t) => {
    const c = (token: string) => parse(t[token]!);
    const surface = c("--surface");
    // Texto sobre el color principal (botón primario).
    expect(contrast(c("--beacon-ink"), c("--beacon"))).toBeGreaterThanOrEqual(4.5);
    // Texto en el color fuerte (enlaces, etiquetas) sobre las superficies y sobre el fondo suave del acento.
    for (const bg of ["--surface", "--surface-2", "--bg"]) expect(contrast(c("--beacon-strong"), c(bg)), bg).toBeGreaterThanOrEqual(4.5);
    expect(contrast(c("--beacon-strong"), over(c("--beacon-soft"), surface))).toBeGreaterThanOrEqual(4.5);
    // Bordes, pestaña seleccionada e indicadores.
    for (const bg of ["--surface", "--surface-2", "--bg"]) expect(contrast(c("--beacon"), c(bg)), bg).toBeGreaterThanOrEqual(3);
    // Anillo de foco.
    expect(contrast(c("--focus"), surface)).toBeGreaterThanOrEqual(3);
    // Acento de la barra lateral (siempre oscura): icono activo sobre el casco y sobre la fila seleccionada.
    expect(contrast(c("--beacon-nav"), c("--hull"))).toBeGreaterThanOrEqual(4.5);
    expect(contrast(c("--beacon-nav"), c("--hull-3"))).toBeGreaterThanOrEqual(3);
  });

  it("el morado solo redefine tokens del acento (sin reglas por componente)", () => {
    for (const sel of [':root[data-accent="purple"]', ':root[data-theme="dark"][data-accent="purple"]']) {
      expect(Object.keys(block(sel)).sort(), sel).toEqual(["--beacon", "--beacon-ink", "--beacon-nav", "--beacon-soft", "--beacon-strong", "--focus"]);
    }
    expect(css.match(/data-accent/g)).toHaveLength(2);
  });

  it("la barra lateral usa el token propio del casco, no el acento general", () => {
    const sidebar = readFileSync(resolve(__dirname, "../app/Sidebar.tsx"), "utf8");
    expect(sidebar).toContain("text-beacon-nav");
    expect(sidebar).not.toMatch(/text-beacon(?![-\w])/);
  });
});
