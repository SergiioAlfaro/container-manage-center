import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { App } from "./App";
import { DEFAULT_CONFIG, useApp } from "./store";
import { renderWithQuery, setServerState } from "@/test/render";

/** Botones sin texto visible (solo icono) que no dicen qué hacen. */
function unnamedIconButtons(root: HTMLElement): string[] {
  return within(root)
    .queryAllByRole("button")
    .filter((b) => (b.textContent ?? "").trim() === "")
    .filter((b) => !b.getAttribute("aria-label") || !b.getAttribute("title"))
    .map((b) => b.outerHTML.slice(0, 160));
}

describe("Accesibilidad de la interacción (0.9.1)", () => {
  beforeEach(() => {
    api.configSet.mockImplementation(async (c) => c);
  });

  it("en todos los apartados, los botones de solo icono tienen nombre accesible y descripción emergente", async () => {
    useApp.setState({ section: "dashboard", config: structuredClone(DEFAULT_CONFIG), saveError: null, info: null });
    setServerState(connected);
    await act(() => useApp.getState().init());
    renderWithQuery(<App />);
    const nav = screen.getByRole("navigation", { name: "Navegación principal" });
    const problems: Record<string, string[]> = {};
    for (const item of within(nav).getAllByRole("button").filter((b) => b.textContent && !/Contraer|Expandir/.test(b.textContent))) {
      await userEvent.click(item);
      await waitFor(() => expect(screen.queryByText("Cargando…")).not.toBeInTheDocument());
      // Deja que lleguen los datos simulados.
      await act(async () => new Promise((r) => setTimeout(r, 50)));
      const found = unnamedIconButtons(document.body);
      if (found.length) problems[item.textContent!] = found;
    }
    // Con la barra contraída los apartados solo muestran el icono.
    await userEvent.click(screen.getByRole("button", { name: "Contraer barra lateral" }));
    const collapsed = within(screen.getByRole("navigation")).getAllByRole("button");
    for (const b of collapsed) expect(b).toHaveAccessibleName();
    expect(problems).toEqual({});
  });
});
