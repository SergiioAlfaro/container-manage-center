// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api } from "@/test/apiMock";
import { linkAllowed, terminalLinkHandler, TerminalLinkPrompt, useLinkPrompt } from "./terminalLinks";

describe("enlaces de las terminales (E2, CMC-SEC-12)", () => {
  it("solo http y https, sin credenciales ni caracteres de control", () => {
    for (const ok of ["https://example.com/a?b#c", "http://localhost:8080/"]) expect(linkAllowed(ok), ok).toBe(true);
    for (const bad of ["file:///etc/passwd", "javascript:alert(1)", "ssh://h", "https://u:p@example.com/", "https://a b", "https://x\n--flag", "HTTPS://example.com", `https://e.com/${"a".repeat(2050)}`]) {
      expect(linkAllowed(bad), bad).toBe(false);
    }
    expect(terminalLinkHandler.allowNonHttpProtocols).toBe(false);
  });

  it("pide confirmación, muestra la dirección y solo la abre al aceptar", async () => {
    render(<TerminalLinkPrompt />);
    // Un enlace no permitido no abre nada.
    act(() => terminalLinkHandler.activate(new MouseEvent("click"), "file:///etc/shadow", {} as never));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    act(() => terminalLinkHandler.activate(new MouseEvent("click"), "https://example.com/informe", {} as never));
    expect(screen.getByRole("dialog", { name: "Abrir el enlace de la terminal" })).toBeInTheDocument();
    expect(screen.getByText("https://example.com/informe")).toBeInTheDocument();
    expect(api.openTerminalLink).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.openTerminalLink).not.toHaveBeenCalled();

    act(() => useLinkPrompt.getState().ask("https://example.com/informe"));
    await userEvent.click(screen.getByRole("button", { name: "Abrir en el navegador" }));
    expect(api.openTerminalLink).toHaveBeenCalledWith("https://example.com/informe");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
