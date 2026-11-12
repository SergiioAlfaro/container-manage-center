import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./controls";

/**
 * Estados de los botones (D-068) con el color principal morado (D-082): todos
 * salen de los tokens del tema (--beacon*), sin colores propios, así que el
 * morado se aplica a hover, active, focus, disabled y loading en claro y oscuro.
 */
describe("Botón: estados con el color principal", () => {
  it("el botón principal usa los tokens del acento en reposo, hover y active", () => {
    render(<Button variant="primary">Crear</Button>);
    const b = screen.getByRole("button", { name: "Crear" });
    expect(b.className).toContain("bg-beacon");
    expect(b.className).toContain("text-beacon-ink");
    expect(b.className).toContain("enabled:hover:bg-beacon-strong");
    expect(b.className).toContain("enabled:active:brightness-95");
    // Ningún color fijo: el estado depende solo de los tokens.
    expect(b.className).not.toMatch(/#[0-9a-f]{3,6}|teal|cyan|emerald|green/i);
  });

  it("deshabilitado: atenuado, sin hover y sin responder al clic", async () => {
    const onClick = vi.fn();
    render(
      <Button variant="primary" disabled onClick={onClick}>
        Siguiente
      </Button>,
    );
    const b = screen.getByRole("button", { name: "Siguiente" });
    expect(b).toBeDisabled();
    expect(b.className).toContain("disabled:opacity-50");
    await userEvent.click(b);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("cargando: indicador giratorio, ocupado para los lectores de pantalla y sin doble envío", async () => {
    const onClick = vi.fn();
    render(
      <Button variant="primary" loading onClick={onClick}>
        Probar conexión
      </Button>,
    );
    const b = screen.getByRole("button", { name: "Probar conexión" });
    expect(b).toHaveAttribute("aria-busy", "true");
    expect(b).toBeDisabled();
    expect(b.querySelector("svg.animate-spin")).not.toBeNull();
    await userEvent.click(b);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("foco: el anillo usa el token --focus (morado) en todos los botones", async () => {
    render(<Button>Actualizar</Button>);
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Actualizar" })).toHaveFocus();
    // :focus-visible { outline: 2px solid var(--focus) } en styles/app.css (morado en claro y oscuro).
    const css = (await import("node:fs")).readFileSync(`${__dirname}/../../styles/app.css`, "utf8");
    expect(css).toMatch(/:focus-visible\s*\{\s*outline:\s*2px solid var\(--focus\)/);
  });
});
