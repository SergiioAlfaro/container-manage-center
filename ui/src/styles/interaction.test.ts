import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// jsdom no aplica :hover ni @media; se comprueba que las reglas globales que
// dan los estados de interacción existen (la comprobación visual se hace con
// la aplicación real: ver el informe de la 0.9.1).
const css = readFileSync(resolve(__dirname, "app.css"), "utf8");

describe("estados de interacción globales (0.9.1)", () => {
  it("foco visible con teclado", () => {
    expect(css).toMatch(/:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--focus\)/);
  });
  it("hover en las filas de las tablas y en los desplegables", () => {
    expect(css).toMatch(/:where\(tbody tr\):hover\s*\{[^}]*background-color:\s*var\(--surface-2\)/);
    expect(css).toMatch(/:where\(summary\):hover/);
  });
  it("transiciones cortas y desactivadas con «reducir movimiento»", () => {
    expect(css).toMatch(/transition-duration:\s*150ms/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)\s*\{[\s\S]*transition-duration:\s*0\.001ms !important/);
  });
  it("los colores de los estados existen en el tema claro y en el oscuro", () => {
    const light = css.slice(0, css.indexOf("[data-theme=\"dark\"]"));
    const dark = css.slice(css.indexOf("[data-theme=\"dark\"]"));
    for (const token of ["--surface-2", "--focus", "--beacon", "--line-strong", "--signal-err"]) {
      expect(light, token).toContain(`${token}:`);
      expect(dark, token).toContain(`${token}:`);
    }
  });
});
