/**
 * Único punto de contacto entre la interfaz y el backend Rust.
 * Los componentes nunca llaman a `invoke` directamente; así las pruebas pueden
 * sustituir este módulo y los tipos se comparten con Rust (bindings/, ts-rs).
 */
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import i18n from "@/shared/i18n";
import type { SystemInfo } from "./bindings/SystemInfo";
import type { UpdateCheck } from "./bindings/UpdateCheck";
import type { UpdateOutcome } from "./bindings/UpdateOutcome";
import type { ActivityEvent } from "./bindings/ActivityEvent";
import type { AppConfig } from "./bindings/AppConfig";
import type { AppInfo } from "./bindings/AppInfo";
import type { CommandError } from "./bindings/CommandError";
import type { OperationRecord } from "./bindings/OperationRecord";
import type { ComposeStatus } from "./bindings/ComposeStatus";
import type { ConnectionState } from "./bindings/ConnectionState";
import type { ConnectionTest } from "./bindings/ConnectionTest";
import type { ContainerAction } from "./bindings/ContainerAction";
import type { ContainerState } from "./bindings/ContainerState";
import type { ContainerSummary } from "./bindings/ContainerSummary";
import type { DockerConfig } from "./bindings/DockerConfig";
import type { ImageSummary } from "./bindings/ImageSummary";
import type { SystemOverview } from "./bindings/SystemOverview";
import type { ContainerDetails } from "./bindings/ContainerDetails";
import type { ContainerSpec } from "./bindings/ContainerSpec";
import type { EditableConfig } from "./bindings/EditableConfig";
import type { EditOutcome } from "./bindings/EditOutcome";
import type { EditPlan } from "./bindings/EditPlan";
import type { EditRequest } from "./bindings/EditRequest";
import type { ApplyOptions } from "./bindings/ApplyOptions";
import type { CommitPreview } from "./bindings/CommitPreview";
import type { CommitRequest } from "./bindings/CommitRequest";
import type { CommitResult } from "./bindings/CommitResult";
import type { CreateResult } from "./bindings/CreateResult";
import type { DuplicateSpec } from "./bindings/DuplicateSpec";
import type { ImageDefaults } from "./bindings/ImageDefaults";
import type { LogEvent } from "./bindings/LogEvent";
import type { NetworkBrief } from "./bindings/NetworkBrief";
import type { ShellChoice } from "./bindings/ShellChoice";
import type { SpecPreview } from "./bindings/SpecPreview";
import type { StatsEvent } from "./bindings/StatsEvent";
import type { TerminalEvent } from "./bindings/TerminalEvent";
import type { VolumeBrief } from "./bindings/VolumeBrief";
import type { BuildEvent } from "./bindings/BuildEvent";
import type { BuildPreview } from "./bindings/BuildPreview";
import type { BuildSpec } from "./bindings/BuildSpec";
import type { ImageDetails } from "./bindings/ImageDetails";
import type { ImageLayer } from "./bindings/ImageLayer";
import type { ConnectOptions } from "./bindings/ConnectOptions";
import type { CreatedNetwork } from "./bindings/CreatedNetwork";
import type { NetworkDetails } from "./bindings/NetworkDetails";
import type { NetworkPreview } from "./bindings/NetworkPreview";
import type { NetworkSpec } from "./bindings/NetworkSpec";
import type { NetworkSummary } from "./bindings/NetworkSummary";
import type { BindSummary } from "./bindings/BindSummary";
import type { VolumeDetails } from "./bindings/VolumeDetails";
import type { VolumePreview } from "./bindings/VolumePreview";
import type { VolumePruneResult } from "./bindings/VolumePruneResult";
import type { VolumeSize } from "./bindings/VolumeSize";
import type { VolumeSpec } from "./bindings/VolumeSpec";
import type { VolumeSummary } from "./bindings/VolumeSummary";
import type { ImageRef } from "./bindings/ImageRef";
import type { PruneResult } from "./bindings/PruneResult";
import type { PullAuth } from "./bindings/PullAuth";
import type { PullEvent } from "./bindings/PullEvent";
import type { RemoveResult } from "./bindings/RemoveResult";
import type { SearchResult } from "./bindings/SearchResult";
import type { ComposeAction } from "./bindings/ComposeAction";
import type { DockerEvent } from "./bindings/DockerEvent";
import type { EventFilter } from "./bindings/EventFilter";
import type { EventPage } from "./bindings/EventPage";
import type { EventQuery } from "./bindings/EventQuery";
import type { EventQueryResult } from "./bindings/EventQueryResult";
import type { HistoryFilter } from "./bindings/HistoryFilter";
import type { HistoryPage } from "./bindings/HistoryPage";
import type { ComposeConfig } from "./bindings/ComposeConfig";
import type { ComposeEvent } from "./bindings/ComposeEvent";
import type { ComposeFile } from "./bindings/ComposeFile";
import type { ComposePreview } from "./bindings/ComposePreview";
import type { ComposeProject } from "./bindings/ComposeProject";
import type { ProjectRef } from "./bindings/ProjectRef";
import type { RegisterResult } from "./bindings/RegisterResult";
import type { UnsavedFile } from "./bindings/UnsavedFile";
import type { ComponentsReport } from "./bindings/ComponentsReport";
import type { RemovalPlan } from "./bindings/RemovalPlan";
import type { RemovalResult } from "./bindings/RemovalResult";
import type { ServerView } from "./bindings/ServerView";
import type { ServerConfig } from "./bindings/ServerConfig";
import type { ProfileIssue } from "./bindings/ProfileIssue";
import type { ConnectOutcome } from "./bindings/ConnectOutcome";
import type { TestReport } from "./bindings/TestReport";
import type { HostKeyInfo } from "./bindings/HostKeyInfo";
import type { TrustedKey } from "./bindings/TrustedKey";
import type { SecretStatus } from "./bindings/SecretStatus";
import type { KeyringStatus } from "./bindings/KeyringStatus";
import type { ServerDiagnosis } from "./bindings/ServerDiagnosis";
import type { ServerOverview } from "./bindings/ServerOverview";
import type { ServerStateEvent } from "./bindings/ServerStateEvent";

export const ACTIVITY_EVENT = "activity://event";
export const ACTIVITY_RESYNC = "activity://resync";
export const ENGINE_STATE = "engine://state";
export const SERVER_STATE = "server://state";

/** Normaliza cualquier error de `invoke` a `CommandError`. */
/**
 * Errores cuyo texto es siempre el mismo (lo genera la aplicación, no Docker):
 * se muestran en el idioma de la interfaz (Fase 9). Los demás conservan el
 * mensaje original, que lleva el detalle técnico (el de Docker, una ruta…).
 */
/** Perfil de GitHub del autor (Ayuda y documentación). */
export const GITHUB_PROFILE = "https://github.com/SergiioAlfaro";

export const LOCALIZED_ERRORS = [
  "engine.notConnected",
  "engine.timeout",
  "engine.notConfirmed",
  "engine.composeUnavailable",
  "ipc.unavailable",
  "updates.noSource",
  "updates.invalidSource",
  "updates.insecureSource",
  "packages.cancelled",
  "packages.denied",
  "packages.busy",
  "packages.notInstalled",
] as const;

export function localizeError(e: CommandError): CommandError {
  if (!(LOCALIZED_ERRORS as readonly string[]).includes(e.code)) return e;
  const key = `errorCodes.${e.code.replace(".", "_")}`;
  return i18n.exists(key) ? { ...e, message: i18n.t(key) } : e;
}

export function toCommandError(e: unknown): CommandError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return localizeError(e as CommandError);
  }
  return { code: "unknown", message: e instanceof Error ? e.message : String(e), httpStatus: null, issues: null };
}

async function callIpc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return call<T>(cmd, args);
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw localizeError({ code: "ipc.unavailable", message: "La interfaz se está ejecutando fuera de la aplicación de escritorio.", httpStatus: null, issues: null });
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toCommandError(e);
  }
}

function channel<T>(handler: (e: T) => void): Channel<T> {
  const ch = new Channel<T>();
  ch.onmessage = handler;
  return ch;
}

/** Equipo local (identificador fijo del backend). */
export const LOCAL_SERVER = "local";

/**
 * Órdenes con Docker de un servidor concreto (0.10.0). Cada llamada lleva el
 * identificador del servidor con el que se creó: nunca depende de un
 * «servidor actual» que pudiera cambiar mientras la operación está en curso.
 */
export function dockerApi(server: string) {
  const call = <T,>(cmd: string, args: Record<string, unknown> = {}) => callIpc<T>(cmd, { ...args, server });
  return {
  engineState: () => call<ConnectionState>("engine_state"),
    engineReconnect: () => call<ConnectionState>("engine_reconnect"),
    engineOverview: () => call<SystemOverview>("engine_overview"),
    composeStatus: () => call<ComposeStatus>("compose_status"),
    containersList: () => call<ContainerSummary[]>("containers_list"),
    containerAction: (id: string, name: string | null, action: ContainerAction) => call<ContainerState>("container_action", { id, name, action }),
    containerRemove: (id: string, name: string | null, force: boolean, volumes: boolean) =>
      call<void>("container_remove", { id, name, force, volumes, confirmed: true }),
    imagesList: () => call<ImageSummary[]>("images_list"),

    containerCreate: (spec: ContainerSpec, start: boolean) => call<CreateResult>("container_create", { spec, start }),
    containerInspect: (id: string, name: string | null) => call<ContainerDetails>("container_inspect", { id, name }),
    containerDuplicateSpec: (id: string, name: string | null) => call<DuplicateSpec>("container_duplicate_spec", { id, name }),
    // 0.9.2: editar configuración, recreación segura, copia de seguridad e imagen desde contenedor.
    containerEditConfig: (id: string) => call<EditableConfig>("container_edit_config", { id }),
    containerEditPlan: (id: string, request: EditRequest) => call<EditPlan>("container_edit_plan", { id, request }),
    containerEditApply: (id: string, request: EditRequest, options: ApplyOptions) => call<EditOutcome>("container_edit_apply", { id, request, options }),
    containerRestoreBackup: (id: string, start: boolean) => call<EditOutcome>("container_restore_backup", { id, start, confirmed: true }),
    containerCommitPreview: (id: string, request: CommitRequest) => call<CommitPreview>("container_commit_preview", { id, request }),
    containerCommit: (id: string, request: CommitRequest) => call<CommitResult>("container_commit", { id, request }),
    imageDefaults: (image: string) => call<ImageDefaults>("image_defaults", { image }),
    networksBrief: () => call<NetworkBrief[]>("networks_brief"),
    volumesBrief: () => call<VolumeBrief[]>("volumes_brief"),

    /** Abre un flujo; devuelve su identificador para cerrarlo con `streamClose`. */
    logsOpen: (id: string, name: string | null, tail: number | null, onEvent: (e: LogEvent) => void) =>
      call<number>("logs_open", { id, name, tail, onEvent: channel(onEvent) }),
    statsOpen: (id: string, name: string | null, onEvent: (e: StatsEvent) => void) => call<number>("stats_open", { id, name, onEvent: channel(onEvent) }),
    terminalOpen: (id: string, name: string | null, shell: ShellChoice, cols: number, rows: number, onEvent: (e: TerminalEvent) => void) =>
      call<number>("terminal_open", { id, name, shell, cols, rows, onEvent: channel(onEvent) }),
    imageDetails: (reference: string) => call<ImageDetails>("image_details", { reference }),
    imageHistory: (reference: string) => call<ImageLayer[]>("image_history", { reference }),
    imageTag: (source: string, repo: string, tag: string) => call<string>("image_tag", { source, repo, tag }),
    imageRemove: (reference: string, force: boolean) => call<RemoveResult>("image_remove", { reference, force, confirmed: true }),
    imagesPrune: (all: boolean) => call<PruneResult>("images_prune", { all, confirmed: true }),
    imagesSearch: (term: string, limit: number, officialOnly: boolean) => call<SearchResult[]>("images_search", { term, limit, officialOnly }),
    /** Descarga en flujo. La contraseña (modo manual) solo viaja en esta llamada. */
    imagePullOpen: (reference: string, auth: PullAuth, onEvent: (e: PullEvent) => void) => call<number>("image_pull_open", { reference, auth, onEvent: channel(onEvent) }),
    buildOpen: (spec: BuildSpec, onEvent: (e: BuildEvent) => void) => call<number>("build_open", { spec, onEvent: channel(onEvent) }),

    networksList: () => call<NetworkSummary[]>("networks_list"),
    networkDetails: (id: string, name: string | null) => call<NetworkDetails>("network_details", { id, name }),
    networkCreate: (spec: NetworkSpec) => call<CreatedNetwork>("network_create", { spec }),
    networkRemove: (id: string, name: string | null) => call<void>("network_remove", { id, name, confirmed: true }),
    networksPrune: () => call<string[]>("networks_prune", { confirmed: true }),
    networkConnect: (network: string, networkName: string | null, container: string, containerName: string | null, options: ConnectOptions, subnets: string[]) =>
      call<void>("network_connect", { network, networkName, container, containerName, options, subnets }),
    networkDisconnect: (network: string, networkName: string | null, container: string, containerName: string | null, force: boolean) =>
      call<void>("network_disconnect", { network, networkName, container, containerName, force, confirmed: true }),

    volumesList: () => call<VolumeSummary[]>("volumes_list"),
    volumeDetails: (name: string) => call<VolumeDetails>("volume_details", { name }),
    /** Tamaños de los volúmenes (`docker system df --verbose`); puede tardar. */
    volumeSizes: () => call<VolumeSize[]>("volume_sizes"),
    volumeCreate: (spec: VolumeSpec) => call<VolumeSummary>("volume_create", { spec }),
    volumeRemove: (name: string) => call<void>("volume_remove", { name, confirmed: true }),
    volumesPrune: (all: boolean) => call<VolumePruneResult>("volumes_prune", { all, confirmed: true }),
    bindsList: () => call<BindSummary[]>("binds_list"),

    // Fase 7: Docker Compose (CLI oficial sin shell).
    composeProjects: () => call<ComposeProject[]>("compose_projects"),
    /** `docker compose config`; con `unsaved` valida el contenido del editor sin guardarlo. */
    composeConfig: (project: ProjectRef, unsaved: UnsavedFile | null = null) => call<ComposeConfig>("compose_config", { project, unsaved }),
    composePreview: (project: ProjectRef, action: ComposeAction) => call<ComposePreview>("compose_preview", { project, action }),
    /** Acción con la salida en vivo; se detiene con `streamClose`. */
    composeRunOpen: (project: ProjectRef, action: ComposeAction, confirmed: boolean, onEvent: (e: ComposeEvent) => void) =>
      call<number>("compose_run_open", { project, action, confirmed, onEvent: channel(onEvent) }),
    composeLogsOpen: (project: ProjectRef, services: string[], tail: number, timestamps: boolean, onEvent: (e: ComposeEvent) => void) =>
      call<number>("compose_logs_open", { project, services, tail, timestamps, onEvent: channel(onEvent) }),
    composeRegister: (files: string[], name: string | null) => call<RegisterResult>("compose_register", { files, name }),
    eventsQuery: (query: EventQuery) => call<EventQueryResult>("events_query", { query }),
  };
}

export type DockerApi = ReturnType<typeof dockerApi>;

const appApi = {
  // 0.10.0: servidores. Cada orden indica su servidor de forma explícita.
  serversList: () => call<ServerView[]>("servers_list"),
  serverValidate: (config: ServerConfig) => call<ProfileIssue[]>("server_validate", { config }),
  serverSave: (config: ServerConfig) => call<ServerConfig>("server_save", { config }),
  /** Solo borra el perfil y sus datos en la aplicación; nada del servidor. */
  serverDelete: (server: string) => call<null>("server_delete", { server, confirmed: true }),
  /** `secret`: frase de paso o contraseña escrita ahora; solo viaja en esta llamada. */
  serverConnect: (server: string, secret: string | null, remember: boolean) => call<ConnectOutcome>("server_connect", { server, secret, remember }),
  serverDisconnect: (server: string) => call<null>("server_disconnect", { server }),
  serverTest: (config: ServerConfig, secret: string | null) => call<TestReport>("server_test", { config, secret }),
  serverHostKey: (config: ServerConfig) => call<HostKeyInfo>("server_host_key", { config }),
  serverTrustedKeys: (config: ServerConfig) => call<TrustedKey[]>("server_trusted_keys", { config }),
  serverTrust: (config: ServerConfig, key: HostKeyInfo, replace: boolean) => call<HostKeyInfo>("server_trust", { config, key, replace }),
  serverSecretStatus: (server: string) => call<SecretStatus>("server_secret_status", { server }),
  keyringStatus: () => call<KeyringStatus>("keyring_status"),
  serverForgetSecret: (server: string) => call<number>("server_forget_secret", { server }),
  serverDiagnose: (server: string) => call<ServerDiagnosis>("server_diagnose", { server }),
  serversOverview: () => call<ServerOverview[]>("servers_overview"),
  onServerState: async (handler: (e: ServerStateEvent) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    return listen<ServerStateEvent>(SERVER_STATE, (e) => handler(e.payload));
  },
  appInfo: () => call<AppInfo>("app_info"),
  /** Fase 9: versiones nuevas (solo consulta) e información del sistema. */
  updatesCheck: (source?: string) => call<UpdateCheck>("updates_check", { source: source ?? null }),
  updatesLast: () => call<UpdateOutcome | null>("updates_last"),
  openReleasePage: (url: string) => call<null>("open_release_page", { url }),
  systemInfo: () => call<SystemInfo>("system_info"),
  /** Abre el perfil de GitHub del autor en el navegador del sistema (plugin
   *  opener de Tauri; la capacidad solo permite esta dirección). */
  openGithubProfile: async () => {
    if (!isTauri()) throw localizeError({ code: "ipc.unavailable", message: "La interfaz se está ejecutando fuera de la aplicación de escritorio.", httpStatus: null, issues: null });
    try {
      await openUrl(GITHUB_PROFILE);
    } catch (e) {
      throw toCommandError(e);
    }
  },
  /** Desinstalación y componentes (0.9.1): consulta y simula, sin cambiar nada. */
  componentsInspect: () => call<ComponentsReport>("components_inspect"),
  componentsPlan: (packages: string[]) => call<RemovalPlan>("components_plan", { packages }),
  /** Desinstala con APT (pide permisos con pkexec). Solo tras la confirmación del usuario. */
  componentsRemove: (packages: string[]) => call<RemovalResult>("components_remove", { packages, confirmed: true }),
  configGet: () => call<AppConfig>("config_get"),
  configSet: (config: AppConfig) => call<AppConfig>("config_set", { config }),
  activityList: () => call<OperationRecord[]>("activity_list"),
  activityClear: () => call<void>("activity_clear"),

  engineTest: (docker: DockerConfig) => call<ConnectionTest>("engine_test", { docker }),
  /** Valida y traduce la especificación sin hablar con Docker. */
  containerPreview: (spec: ContainerSpec) => call<SpecPreview>("container_preview", { spec }),
  terminalInput: (stream: number, data: string) => call<void>("terminal_input", { stream, data }),
  terminalResize: (stream: number, cols: number, rows: number) => call<void>("terminal_resize", { stream, cols, rows }),
  streamClose: (stream: number) => call<boolean>("stream_close", { stream }),

  /** Interpreta una referencia como Docker, sin hablar con Docker. */
  imageRefParse: (reference: string) => call<ImageRef>("image_ref_parse", { reference }),
  /** Valida y resume el contexto sin hablar con Docker. */
  buildPreview: (spec: BuildSpec) => call<BuildPreview>("build_preview", { spec }),
  /** Valida la red y muestra la petición exacta sin hablar con Docker. */
  networkPreview: (spec: NetworkSpec) => call<NetworkPreview>("network_preview", { spec }),
  /** Valida el volumen y muestra la petición exacta sin hablar con Docker. */
  volumePreview: (spec: VolumeSpec) => call<VolumePreview>("volume_preview", { spec }),
  composeUnregister: (name: string) => call<boolean>("compose_unregister", { name }),
  composeFileRead: (path: string) => call<ComposeFile>("compose_file_read", { path }),
  composeFileSave: (path: string, content: string, version: string) => call<ComposeFile>("compose_file_save", { path, content, version }),
  composeFileCreate: (dir: string, content: string) => call<ComposeFile>("compose_file_create", { dir, content }),
  composeFindFile: (dir: string) => call<string | null>("compose_find_file", { dir }),

  // Fase 8: eventos de Docker e historial de operaciones.
  eventsRecent: (filter: Partial<EventFilter> = {}) => call<EventPage>("events_recent", { filter }),
  eventsJournal: (filter: Partial<EventFilter> = {}) => call<EventPage>("events_journal", { filter }),
  eventsJournalClear: () => call<null>("events_journal_clear"),
  historyList: (filter: Partial<HistoryFilter> = {}) => call<HistoryPage>("history_list", { filter }),
  historyClear: () => call<null>("history_clear"),

  /** Selector de archivo del sistema (Dockerfile). */
  pickFile: async (title: string, defaultPath?: string): Promise<string | null> => {
    if (!isTauri()) return null;
    const r = await openDialog({ directory: false, multiple: false, title, defaultPath });
    return typeof r === "string" ? r : null;
  },

  /** Selector de uno o varios archivos YAML (proyectos Compose). */
  pickComposeFiles: async (title: string, filterName: string): Promise<string[]> => {
    if (!isTauri()) return [];
    const r = await openDialog({ directory: false, multiple: true, title, filters: [{ name: filterName, extensions: ["yaml", "yml"] }] });
    if (Array.isArray(r)) return r;
    return typeof r === "string" ? [r] : [];
  },

  /** Selector de carpeta del sistema (para los bind mounts). */
  pickFolder: async (title: string): Promise<string | null> => {
    if (!isTauri()) return null;
    const r = await openDialog({ directory: true, multiple: false, title });
    return typeof r === "string" ? r : null;
  },

  onEngineState: async (handler: (s: ConnectionState) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    return listen<ConnectionState>(ENGINE_STATE, (e) => handler(e.payload));
  },

  /** Resultado de la comprobación de versiones nuevas al iniciar. */
  onUpdateResult: async (handler: (o: UpdateOutcome) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    return listen<UpdateOutcome>("updates://result", (e) => handler(e.payload));
  },

  /** Cada evento de Docker recibido por la suscripción de la aplicación. */
  onDockerEvent: async (handler: (e: DockerEvent) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    return listen<DockerEvent>("docker://event", (e) => handler(e.payload));
  },

  onActivity: async (handler: (e: ActivityEvent) => void, onResync: () => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    const un1 = await listen<ActivityEvent>(ACTIVITY_EVENT, (e) => handler(e.payload));
    const un2 = await listen(ACTIVITY_RESYNC, () => onResync());
    return () => {
      un1();
      un2();
    };
  },

  copyText: async (text: string): Promise<void> => {
    if (isTauri()) return writeText(text);
    return navigator.clipboard.writeText(text);
  },
};

/**
 * Órdenes de la aplicación (sin servidor). Las de Docker se obtienen siempre
 * para un servidor concreto: `useApi()` dentro de su ámbito (app/server.tsx)
 * o `dockerApi(servidor)` donde se guarda el servidor de la operación.
 */
export const api = appApi;

export type Api = typeof api;
