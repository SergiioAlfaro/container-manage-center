// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Color principal (0.9.2, D-072; 1.0.0, D-082: solo morado): los contrastes se
 * calculan con la fórmula de WCAG 2.x a partir de los tokens reales de
 * app.css, en los dos temas. Umbrales: 4.5:1 para texto y 3:1 para elementos de interfaz
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
  "morado · claro": light,
  "morado · oscuro": dark,
};

/** Tono (0–360) de un color opaco. */
function hue([r, g, b]: Rgba): number {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B);
  const d = max - Math.min(R, G, B);
  if (d === 0) return 0;
  const h = max === R ? ((G - B) / d) % 6 : max === G ? (B - R) / d + 2 : (R - G) / d + 4;
  return (h * 60 + 360) % 360;
}
const ACCENT_TOKENS = ["--beacon", "--beacon-strong", "--beacon-soft", "--beacon-nav", "--focus"];

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

  it("no queda ningún verde como color principal: los tokens del acento son morados", () => {
    for (const [name, t] of Object.entries(THEMES)) {
      for (const token of ACCENT_TOKENS) {
        const h = hue(parse(t[token]!));
        // Morado/violeta: entre 250° y 290°. El verde azulado original estaba en ~182°.
        expect(h, `${name} ${token}`).toBeGreaterThanOrEqual(250);
        expect(h, `${name} ${token}`).toBeLessThanOrEqual(290);
      }
    }
    // Ni selectores por color ni los valores del verde azulado anterior.
    expect(css).not.toMatch(/data-accent/);
    for (const old of ["#0d8287", "#0a6b6f", "#25b0b3", "#4cc6c8", "13 130 135", "37 176 179"]) expect(css.toLowerCase()).not.toContain(old);
  });

  it("los colores semánticos (éxito, ejecución, aviso, error) no cambian", () => {
    expect(Object.fromEntries(["--signal-ok", "--signal-run", "--signal-warn", "--signal-err", "--signal-off"].map((k) => [k, light[k]]))).toEqual({
      "--signal-ok": "#2b8a57",
      "--signal-run": "#2a74b0",
      "--signal-warn": "#b7821b",
      "--signal-err": "#bf412e",
      "--signal-off": "#8794a2",
    });
    expect(Object.fromEntries(["--signal-ok", "--signal-run", "--signal-warn", "--signal-err", "--signal-off"].map((k) => [k, dark[k]]))).toEqual({
      "--signal-ok": "#4fb57c",
      "--signal-run": "#5b9fd6",
      "--signal-warn": "#d9a63c",
      "--signal-err": "#e0664f",
      "--signal-off": "#6c7d8f",
    });
    // El éxito sigue siendo verde (es un indicador funcional, no el color principal).
    expect(hue(parse(light["--signal-ok"]!))).toBeGreaterThan(90);
    expect(hue(parse(light["--signal-ok"]!))).toBeLessThan(160);
  });

  it("la barra lateral usa el token propio del casco, no el acento general", () => {
    const sidebar = readFileSync(resolve(__dirname, "../app/Sidebar.tsx"), "utf8");
    expect(sidebar).toContain("text-beacon-nav");
    expect(sidebar).not.toMatch(/text-beacon(?![-\w])/);
  });
});
