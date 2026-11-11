import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, commitPreviewOf, connected, editableConfig, editOutcome, editPlanOf } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { useApp } from "@/app/store";
import { useImagesNav } from "@/features/images/store";
import type { EditRequest } from "@/shared/ipc/bindings/EditRequest";
import { useContainersNav } from "../store";
import { ContainersView } from "../ContainersView";
import { CommitDialog, commitCommand, fullReference, localCommitIssues } from "./CommitDialog";
import { EditContainer } from "./EditContainer";
import { pendingByTab, pendingChanges, tabOfIssue, textsOf, withTexts } from "./model";

const base = () => structuredClone(editableConfig().current);
const commitReq = (over: Partial<Parameters<typeof localCommitIssues>[0]> = {}) => ({ repository: "", tag: "", author: "", message: "", pause: true, replaceExisting: false, ...over });

describe("Editar configuración: modelo", () => {
  it("sin cambios no hay nada pendiente", () => {
    expect(pendingChanges(base(), base())).toEqual([]);
  });

  it("clasifica como el backend: nombre, reinicio y límites directos; el resto exige recrear", () => {
    const next: EditRequest = { ...base(), name: "web2", restart: { name: "always", maxRetries: 0 }, cpus: 2, memoryBytes: 1_073_741_824 };
    expect(pendingChanges(base(), next)).toEqual([
      { field: "name", kind: "direct" },
      { field: "restart", kind: "direct" },
      { field: "cpus", kind: "direct" },
      { field: "memory", kind: "direct" },
    ]);
    const recreate: EditRequest = {
      ...base(),
      description: "nota",
      env: [...base().env, { key: "NUEVA", value: "1" }],
      ports: [{ hostIp: "", hostPort: 9090, containerPort: 80, protocol: "tcp" }],
      mounts: base().mounts.slice(0, 1),
    };
    expect(pendingChanges(base(), recreate).map((p) => [p.field, p.kind])).toEqual([
      ["description", "recreate"],
      ["env", "recreate"],
      ["ports", "recreate"],
      ["mounts", "recreate"],
    ]);
  });

  it("quitar un límite exige recrear (docker update no sabe quitarlo)", () => {
    expect(pendingChanges(base(), { ...base(), cpus: null })).toEqual([{ field: "cpus", kind: "recreate" }]);
    expect(pendingChanges(base(), { ...base(), memoryBytes: null })).toEqual([{ field: "memory", kind: "recreate" }]);
  });

  it("el orden de puertos y montajes no es un cambio", () => {
    const b = { ...base(), mounts: [...base().mounts].reverse() };
    expect(pendingChanges(base(), b)).toEqual([]);
    const two = { ...base(), ports: [...base().ports, { hostIp: "", hostPort: 8443, containerPort: 443, protocol: "tcp" as const }] };
    expect(pendingChanges(two, { ...two, ports: [...two.ports].reverse() })).toEqual([]);
  });

  it("marca las pestañas con cambios y si alguno exige recrear", () => {
    const p = pendingChanges(base(), { ...base(), name: "x", env: [], cpus: 3 });
    expect(pendingByTab(p)).toEqual({ general: "direct", env: "recreate", resources: "direct" });
  });

  it("los números mal escritos se señalan y no se envían", () => {
    const t = textsOf(base());
    expect(t.cpus).toBe("1.5");
    const ok = withTexts(base(), { ...t, cpus: "0,5" });
    expect(ok.problems).toEqual([]);
    expect(ok.request.cpus).toBe(0.5);
    const bad = withTexts(base(), { ...t, cpus: "-1", mem: { value: "abc", unit: "m" } });
    expect(bad.problems).toEqual(["cpus", "memory"]);
    expect(bad.request.cpus).toBe(1.5);
    expect(bad.request.memoryBytes).toBe(536_870_912);
    expect(withTexts(base(), { ...t, cpus: "" }).request.cpus).toBeNull();
  });

  it("cada problema de validación lleva a su pestaña", () => {
    expect(tabOfIssue("ports.0.hostPort")).toBe("ports");
    expect(tabOfIssue("env.2.key")).toBe("env");
    expect(tabOfIssue("mounts.1.source")).toBe("storage");
    expect(tabOfIssue("memoryBytes")).toBe("resources");
    expect(tabOfIssue("name")).toBe("general");
  });
});

describe("Crear imagen: validación y vista previa", () => {
  it("valida repositorio, etiqueta, autor y mensaje", () => {
    expect(localCommitIssues(commitReq()).map((i) => i.code)).toEqual(["commit.repositoryRequired"]);
    expect(localCommitIssues(commitReq({ repository: "mi-app" }))).toEqual([]);
    expect(localCommitIssues(commitReq({ repository: "registro.local:5000/equipo/app", tag: "v1.2" }))).toEqual([]);
    expect(localCommitIssues(commitReq({ repository: "Mayusculas" })).map((i) => i.code)).toEqual(["commit.repositoryInvalid"]);
    expect(localCommitIssues(commitReq({ repository: "a//b" })).map((i) => i.code)).toEqual(["commit.repositoryInvalid"]);
    expect(localCommitIssues(commitReq({ repository: "app", tag: "-mal" })).map((i) => i.code)).toEqual(["commit.tagInvalid"]);
    expect(localCommitIssues(commitReq({ repository: "app", message: "a\nb" })).map((i) => [i.field, i.code])).toEqual([["message", "commit.controlChars"]]);
    expect(localCommitIssues(commitReq({ repository: "app", author: "x".repeat(201) })).map((i) => [i.field, i.code])).toEqual([["author", "commit.tooLong"]]);
  });

  it("muestra el nombre completo y la orden equivalente, sin shell", () => {
    expect(fullReference(commitReq({ repository: " app " }))).toBe("app:latest");
    expect(fullReference(commitReq({ repository: "app", tag: "v2" }))).toBe("app:v2");
    expect(fullReference(commitReq())).toBe("");
    expect(commitCommand(commitReq({ repository: "app", message: "con espacios" }), "web", true, "app:latest")).toBe("docker commit --message 'con espacios' web app:latest");
    expect(commitCommand(commitReq({ repository: "app", pause: false }), "web", true, "app:latest")).toBe("docker commit --pause=false web app:latest");
    // Detenido: no hay nada que pausar.
    expect(commitCommand(commitReq({ repository: "app", pause: false }), "web", false, "app:latest")).toBe("docker commit web app:latest");
  });
});

describe("Crear imagen: diálogo", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
  });

  it("explica qué no entra en la imagen y avisa de que el contenedor está en ejecución", async () => {
    renderWithQuery(<CommitDialog id="a1b2c3d4e5f6a7b8c9d0" name="web" onClose={() => {}} />);
    const dialog = screen.getByRole("dialog", { name: "Crear imagen a partir de web" });
    expect(within(dialog).getByText("La imagen no incluye los volúmenes ni las carpetas montadas")).toBeInTheDocument();
    const list = await within(dialog).findByRole("list", { name: "Montajes que no entran en la imagen" });
    expect(within(list).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["/var/lib/app (volumen: datos)", "/usr/share/nginx/html (carpeta: /srv/web)"]);
    expect(within(dialog).getByText("El contenedor está en ejecución. Sus procesos no se detienen.")).toBeInTheDocument();
    // Las variables de entorno sí entran en la imagen (docker commit guarda la configuración).
    expect(within(dialog).getByText(/La imagen también guarda la configuración del contenedor, incluidas sus variables de entorno/)).toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: /Pausar el contenedor/ })).toBeChecked();
  });

  it("valida antes de enviar y crea la imagen con los datos indicados; se puede abrir en Imágenes", async () => {
    const onClose = vi.fn();
    const showDetail = vi.spyOn(useImagesNav.getState(), "showDetail");
    renderWithQuery(<CommitDialog id="a1b2c3d4e5f6a7b8c9d0" name="web" onClose={onClose} />);
    const dialog = screen.getByRole("dialog");
    await within(dialog).findByRole("list", { name: "Montajes que no entran en la imagen" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Crear imagen" }));
    expect(within(dialog).getByText("Indica el nombre del repositorio.")).toBeInTheDocument();
    expect(api.containerCommit).not.toHaveBeenCalled();

    await userEvent.type(within(dialog).getByLabelText(/^Repositorio/), "mi-app");
    await userEvent.type(within(dialog).getByLabelText(/^Etiqueta/), "v1");
    await userEvent.type(within(dialog).getByLabelText(/^Mensaje/), "Versión de prueba");
    expect(within(dialog).getByText("mi-app:v1")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("checkbox", { name: /Pausar el contenedor/ }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Crear imagen" }));
    await screen.findByRole("dialog", { name: "Imagen creada" });
    expect(api.containerCommit).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", { repository: "mi-app", tag: "v1", author: "", message: "Versión de prueba", pause: false, replaceExisting: false });
    expect(screen.getByText("El contenedor web sigue como estaba.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Ver la imagen" }));
    expect(showDetail).toHaveBeenCalledWith("sha256:c0mm17c0mm17c0mm17", "mi-app:v1");
    expect(useApp.getState().section).toBe("images");
    expect(onClose).toHaveBeenCalled();
  });

  it("no sustituye una imagen existente sin confirmación expresa", async () => {
    api.containerCommitPreview.mockImplementation(async (_id, r) => commitPreviewOf(r, r.repository ? { existingImageId: "sha256:01d01d01d01d" } : {}));
    renderWithQuery(<CommitDialog id="a1b2c3d4e5f6a7b8c9d0" name="web" onClose={() => {}} />);
    const dialog = screen.getByRole("dialog");
    await within(dialog).findByRole("list", { name: "Montajes que no entran en la imagen" });
    await userEvent.type(within(dialog).getByLabelText(/^Repositorio/), "nginx");
    await userEvent.click(within(dialog).getByRole("button", { name: "Crear imagen" }));
    expect(await within(dialog).findByText(/Ya existe una imagen llamada nginx:latest/)).toBeInTheDocument();
    expect(api.containerCommit).not.toHaveBeenCalled();
    expect(within(dialog).getByRole("button", { name: "Crear imagen" })).toBeDisabled();
    await userEvent.click(within(dialog).getByRole("checkbox", { name: /Usar este nombre/ }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Crear imagen" }));
    await screen.findByRole("dialog", { name: "Imagen creada" });
    expect(api.containerCommit).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", expect.objectContaining({ repository: "nginx", replaceExisting: true }));
    api.containerCommitPreview.mockImplementation(async (_id, r) => commitPreviewOf(r));
  });

  it("con el contenedor detenido no ofrece pausar", async () => {
    api.containerCommitPreview.mockImplementation(async (_id, r) => commitPreviewOf(r, { status: "exited" }));
    renderWithQuery(<CommitDialog id="b2b2b2b2b2b2" name="db" onClose={() => {}} />);
    await screen.findByRole("list", { name: "Montajes que no entran en la imagen" });
    expect(screen.queryByRole("checkbox", { name: /Pausar/ })).not.toBeInTheDocument();
    api.containerCommitPreview.mockImplementation(async (_id, r) => commitPreviewOf(r));
  });
});

describe("Editar configuración: pantalla", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
    api.containerEditConfig.mockImplementation(async () => editableConfig());
    api.containerEditPlan.mockImplementation(async (id, r) => editPlanOf(id, r));
  });

  const open = async () => {
    renderWithQuery(<EditContainer id="a1b2c3d4e5f6a7b8c9d0" name="web" />);
    return screen.findByRole("tablist", { name: "Secciones de la configuración" });
  };

  it("tiene las siete pestañas y distingue cambios directos de los que exigen recrear", async () => {
    const tabs = await open();
    expect(within(tabs).getAllByRole("tab").map((t) => t.textContent)).toEqual(["General", "Red", "Almacenamiento", "Variables", "Puertos", "Recursos", "Avanzado"]);
    expect(screen.getByText("Sin cambios pendientes.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Revisar cambios" })).toBeDisabled();
    await userEvent.selectOptions(screen.getByLabelText(/^Política de reinicio/), "always");
    expect(screen.getByText("1 cambio pendiente. Se aplica directamente, sin recrear el contenedor.")).toBeInTheDocument();
    expect(within(screen.getByRole("tab", { name: /General/ })).getByLabelText("Cambio directo")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: /Variables/ }));
    await userEvent.click(screen.getByRole("button", { name: "Añadir variable" }));
    expect(screen.getByText("2 cambios pendientes. Alguno requiere recrear el contenedor.")).toBeInTheDocument();
    expect(within(screen.getByRole("tab", { name: /Variables/ })).getByLabelText("Requiere recrear")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Descartar cambios" }));
    expect(screen.getByText("Sin cambios pendientes.")).toBeInTheDocument();
  });

  it("las variables secretas se muestran ocultas", async () => {
    await open();
    await userEvent.click(screen.getByRole("tab", { name: /Variables/ }));
    const values = screen.getAllByLabelText(/^Valor/);
    expect(values.map((v) => v.getAttribute("type"))).toEqual(["text", "password"]);
  });

  it("la pestaña Avanzado es solo de lectura: no hay campos editables", async () => {
    await open();
    await userEvent.click(screen.getByRole("tab", { name: /Avanzado/ }));
    const panel = screen.getByRole("tabpanel");
    expect(within(panel).getByText("Docker no permite cambiar estas opciones en un contenedor que ya existe.")).toBeInTheDocument();
    expect(within(panel).queryAllByRole("textbox")).toEqual([]);
    expect(within(panel).queryAllByRole("combobox")).toEqual([]);
    expect(within(panel).queryAllByRole("checkbox")).toEqual([]);
    expect(within(panel).getByText("nginx:1.27")).toBeInTheDocument();
  });

  it("un cambio directo se revisa y se aplica sin pedir confirmación de recreación", async () => {
    await open();
    await userEvent.clear(screen.getByLabelText(/^Nombre/));
    await userEvent.type(screen.getByLabelText(/^Nombre/), "web2");
    await userEvent.click(screen.getByRole("button", { name: "Revisar cambios" }));
    expect(await screen.findByRole("heading", { name: "Revisar los cambios" })).toBeInTheDocument();
    expect(screen.getByText("Cambios directos, sin recrear")).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Comparación de la configuración" });
    const row = within(table).getByRole("rowheader", { name: "Nombre" }).closest("tr")!;
    expect(within(row).getByText("web")).toBeInTheDocument();
    expect(within(row).getByText("web2")).toBeInTheDocument();
    expect(within(row).getByText("Cambio directo")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Recreación segura" })).not.toBeInTheDocument();
    expect(api.containerEditPlan).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole("button", { name: "Aplicar cambios" }));
    expect(await screen.findByText("Cambios aplicados a web2 y comprobados.")).toBeInTheDocument();
    expect(api.containerEditApply).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", expect.objectContaining({ name: "web2" }), { confirmRecreate: false, acceptedRisks: [], keepMac: false });
  });

  it("recrear exige aceptar cada riesgo y confirmar; no se aplica nada antes", async () => {
    api.containerEditApply.mockResolvedValueOnce(
      editOutcome({
        containerId: "n3wn3wn3wn3w",
        recreated: true,
        backupId: "a1b2c3d4e5f6a7b8c9d0",
        backupName: "web-dcc-backup-20261111-120000",
        steps: ["stop", "backup", "create", "start", "verify"].map((code) => ({ code, ok: true, detail: null })),
      }),
    );
    await open();
    await userEvent.click(screen.getByRole("tab", { name: /Puertos/ }));
    const hostPort = screen.getAllByRole("textbox").find((i) => (i as HTMLInputElement).value === "8080")!;
    await userEvent.clear(hostPort);
    await userEvent.type(hostPort, "9090");
    await userEvent.click(screen.getByRole("button", { name: "Revisar cambios" }));
    expect(await screen.findByRole("heading", { name: "Recreación segura" })).toBeInTheDocument();
    expect(screen.getByText("Requiere recrear el contenedor")).toBeInTheDocument();
    expect(screen.getByText(/Se le cambia el nombre a web-dcc-backup-20261111-120000/)).toBeInTheDocument();
    expect(screen.getByText(/Se crea un contenedor nuevo llamado web, con la imagen nginx:1.27/)).toBeInTheDocument();
    expect(screen.getByText(/No se elimina ningún volumen, carpeta ni imagen/)).toBeInTheDocument();
    expect(screen.getByText(/El servicio se interrumpirá/)).toBeInTheDocument();
    const recreate = screen.getByRole("button", { name: "Recrear contenedor" });
    expect(recreate).toBeDisabled();
    await userEvent.click(screen.getByRole("checkbox", { name: /He revisado los cambios y quiero recrear web/ }));
    // Falta aceptar el riesgo de la capa de escritura.
    expect(recreate).toBeDisabled();
    await userEvent.click(screen.getByRole("checkbox", { name: /escritos fuera de sus volúmenes/ }));
    expect(recreate).toBeEnabled();
    expect(api.containerEditApply).not.toHaveBeenCalled();
    await userEvent.click(recreate);
    expect(await screen.findByText("web se ha recreado con los cambios y funciona correctamente.")).toBeInTheDocument();
    expect(screen.getByText(/se conserva detenido como web-dcc-backup-20261111-120000/)).toBeInTheDocument();
    expect(api.containerEditApply).toHaveBeenCalledWith(
      "a1b2c3d4e5f6a7b8c9d0",
      expect.objectContaining({ ports: [{ hostIp: "", hostPort: 9090, containerPort: 80, protocol: "tcp" }] }),
      { confirmRecreate: true, acceptedRisks: ["writableLayer"], keepMac: false },
    );
    expect(screen.getAllByLabelText("Correcto")).toHaveLength(5);
    // La cabecera y «Volver al contenedor» pasan al contenedor nuevo (el ID anterior es ahora la copia).
    useContainersNav.setState({ page: { kind: "list" } });
    await userEvent.click(screen.getByRole("button", { name: "Volver al contenedor" }));
    expect(useContainersNav.getState().page).toMatchObject({ kind: "detail", id: "n3wn3wn3wn3w", name: "web" });
  });

  it("si al recrear también cambia el nombre, la revisión y la cabecera usan el nombre nuevo", async () => {
    api.containerEditApply.mockImplementationOnce(async (_id, r) =>
      editOutcome({ containerId: "n3wn3wn3wn3w", name: r.name, recreated: true, backupName: "web-dcc-backup-20261111-120000", steps: [] }),
    );
    await open();
    await userEvent.clear(screen.getByLabelText(/^Nombre/));
    await userEvent.type(screen.getByLabelText(/^Nombre/), "web2");
    await userEvent.type(screen.getByLabelText(/^Descripción/), "nota");
    await userEvent.click(screen.getByRole("button", { name: "Revisar cambios" }));
    expect(await screen.findByText(/Se crea un contenedor nuevo llamado web2,/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: /escritos fuera de sus volúmenes/ }));
    await userEvent.click(screen.getByRole("checkbox", { name: /quiero recrear/ }));
    await userEvent.click(screen.getByRole("button", { name: "Recrear contenedor" }));
    expect(await screen.findByRole("heading", { level: 2, name: "Editar configuración de web2" })).toBeInTheDocument();
  });

  it("si un motivo impide recrear con seguridad, no se ofrece continuar", async () => {
    api.containerEditPlan.mockImplementationOnce(async (id, r) => ({ ...editPlanOf(id, r), blockers: [{ code: "composeManaged", params: { project: "blog" } }] }));
    await open();
    await userEvent.type(screen.getByLabelText(/^Descripción/), "nota");
    await userEvent.click(screen.getByRole("button", { name: "Revisar cambios" }));
    expect(await screen.findByText(/pertenece al proyecto Compose «blog»/)).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: /quiero recrear/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Recrear contenedor" })).toBeDisabled();
  });

  it("si la recreación falla, informa de que se volvió al original", async () => {
    api.containerEditApply.mockResolvedValueOnce(
      editOutcome({
        recreated: false,
        rolledBack: true,
        error: "Docker: port is already allocated",
        steps: [
          { code: "stop", ok: true, detail: null },
          { code: "start", ok: false, detail: "port is already allocated" },
          { code: "rollbackRemove", ok: true, detail: null },
          { code: "rollbackRename", ok: true, detail: null },
          { code: "rollbackStart", ok: true, detail: null },
        ],
      }),
    );
    api.containerEditPlan.mockImplementationOnce(async (id, r) => ({ ...editPlanOf(id, r), risks: [] }));
    await open();
    await userEvent.type(screen.getByLabelText(/^Descripción/), "nota");
    await userEvent.click(screen.getByRole("button", { name: "Revisar cambios" }));
    await userEvent.click(await screen.findByRole("checkbox", { name: /quiero recrear/ }));
    await userEvent.click(screen.getByRole("button", { name: "Recrear contenedor" }));
    expect(await screen.findByText(/Se ha vuelto al contenedor original/)).toBeInTheDocument();
    expect(screen.getAllByLabelText("Error")).toHaveLength(1);
  });

  it("las copias de seguridad se restauran con confirmación", async () => {
    api.containerEditConfig.mockImplementation(async () =>
      editableConfig({ backups: [{ id: "bacbacbacbac", name: "web-dcc-backup-20261110-090000", originalName: "web", state: "exited", created: 1_794_000_000 }] }),
    );
    api.containerRestoreBackup.mockResolvedValueOnce(editOutcome({ containerId: "bacbacbacbac", name: "web" }));
    await open();
    await userEvent.click(screen.getByRole("button", { name: "Restaurar" }));
    const dialog = screen.getByRole("dialog", { name: "¿Restaurar web-dcc-backup-20261110-090000?" });
    expect(within(dialog).getByText(/No se elimina ningún contenedor, volumen ni imagen/)).toBeInTheDocument();
    expect(api.containerRestoreBackup).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Restaurar" }));
    await waitFor(() => expect(api.containerRestoreBackup).toHaveBeenCalledWith("bacbacbacbac", true));
    await waitFor(() => expect(useContainersNav.getState().page).toEqual({ kind: "edit", id: "bacbacbacbac", name: "web" }));
  });

  it("eliminar una copia nunca borra sus volúmenes", async () => {
    api.containerEditConfig.mockImplementation(async () =>
      editableConfig({ backups: [{ id: "bacbacbacbac", name: "web-dcc-backup-20261110-090000", originalName: "web", state: "exited", created: 1_794_000_000 }] }),
    );
    await open();
    await userEvent.click(screen.getByRole("button", { name: "Eliminar copia" }));
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Eliminar copia" }));
    await waitFor(() => expect(api.containerRemove).toHaveBeenCalledWith("bacbacbacbac", "web-dcc-backup-20261110-090000", false, false));
  });
});

describe("Contenedores: menú de acciones de la fila", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
    useContainersNav.setState({ page: { kind: "list" } });
  });

  it("se abre con el botón o con el botón derecho, se maneja con el teclado y lleva a editar y a crear imagen", async () => {
    renderWithQuery(<ContainersView />);
    await screen.findByText("web", { selector: "span" });
    const trigger = screen.getByRole("button", { name: "Más acciones de web" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    await userEvent.click(trigger);
    const menu = screen.getByRole("menu", { name: "Más acciones de web" });
    const items = within(menu).getAllByRole("menuitem");
    expect(items.map((i) => i.textContent)).toEqual(["Editar configuración", "Crear imagen a partir de este contenedor"]);
    expect(items[0]).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(items[1]).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(items[0]).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(items[1]).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();

    // Botón derecho sobre la fila.
    fireEvent.contextMenu(screen.getByText("db", { selector: "span" }).closest("tr")!);
    await userEvent.click(within(screen.getByRole("menu", { name: "Más acciones de db" })).getByRole("menuitem", { name: "Crear imagen a partir de este contenedor" }));
    expect(screen.getByRole("dialog", { name: "Crear imagen a partir de db" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));

    await userEvent.click(screen.getByRole("button", { name: "Más acciones de web" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Editar configuración" }));
    expect(useContainersNav.getState().page).toEqual({ kind: "edit", id: "a1b2c3d4e5f6a7b8c9d0", name: "web" });
  });

  it("un clic fuera cierra el menú", async () => {
    renderWithQuery(<ContainersView />);
    await screen.findByText("web", { selector: "span" });
    await userEvent.click(screen.getByRole("button", { name: "Más acciones de web" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    await act(async () => {
      fireEvent.mouseDown(document.body);
    });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
