// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);
import { api, connected } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { LEGAL, LegalSettings } from "./LegalSettings";

describe("Configuración › Legal (1.7.0)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
  });

  it("muestra nombre, versión, licencia SPDX y copyright", async () => {
    renderWithQuery(<LegalSettings />);
    expect(screen.getByRole("heading", { name: "Legal" })).toBeInTheDocument();
    const facts = screen.getByTestId("legal-facts");
    expect(within(facts).getByText("Container Manage Center (CMC)")).toBeInTheDocument();
    expect(within(facts).getByText("GPL-3.0-or-later")).toBeInTheDocument();
    expect(within(facts).getByText("Copyright © 2026 Sergio Alfaro Lorite")).toBeInTheDocument();
    expect(await within(facts).findByText("0.9.0")).toBeInTheDocument();
  });

  it("incluye la declaración GPL, el enlace a la licencia, los avisos de terceros y la independencia de Docker, Inc.", async () => {
    renderWithQuery(<LegalSettings />);
    expect(screen.getByText(/Licencia Pública General de GNU/)).toBeInTheDocument();
    expect(screen.getByText(/SIN NINGUNA GARANTÍA/)).toBeInTheDocument();
    expect(screen.getByText(LEGAL.licenseUrl)).toBeInTheDocument();
    expect(screen.getByText(LEGAL.notices)).toBeInTheDocument();
    expect(screen.getByText(/no está afiliado a Docker, Inc\./)).toBeInTheDocument();
    // La URL se copia; no se abre ningún programa.
    await userEvent.click(screen.getAllByRole("button", { name: "Copiar" })[0]!);
    expect(api.copyText).toHaveBeenCalledWith(LEGAL.licenseUrl);
  });
});
