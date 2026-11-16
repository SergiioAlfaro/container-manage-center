// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useQuery } from "@tanstack/react-query";
import { useApi } from "@/app/server";
import { useTranslation } from "react-i18next";
import { useEngine } from "@/features/engine/store";
import { formatBytes } from "@/shared/format";
import type { ComposeStatus } from "@/shared/ipc/bindings/ComposeStatus";
import type { SystemInfo } from "@/shared/ipc/bindings/SystemInfo";
import type { SystemOverview } from "@/shared/ipc/bindings/SystemOverview";
import { keys } from "@/shared/query";
import { CopyButton } from "@/shared/ui/controls";
import { Group } from "./layout";
import { isWindows } from "@/shared/platform";

type Line = [label: string, value: string | null | undefined];

function composeText(c: ComposeStatus | undefined, t: (k: string) => string): string | null {
  if (!c) return null;
  return c.status === "available" ? `${c.version} (${c.program})` : t(`system.compose.${c.status}`);
}

/** Informe en texto para copiar (p. ej. al comunicar un problema). Solo datos
 *  del sistema y versiones: sin rutas de proyectos, variables ni credenciales. */
export function systemReport(sections: Array<[string, Line[]]>): string {
  return sections
    .map(([title, lines]) => [`## ${title}`, ...lines.map(([k, v]) => `${k}: ${v ?? "—"}`)].join("\n"))
    .join("\n\n");
}

export function SystemSettings() {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const state = useEngine((s) => s.state);
  const connected = state?.state === "connected";
  const sys = useQuery({ queryKey: ["systemInfo"], queryFn: api.systemInfo, staleTime: Infinity });
  const overview = useQuery<SystemOverview>({ queryKey: keys.overview, queryFn: api.engineOverview, enabled: connected });
  const compose = useQuery({ queryKey: keys.compose, queryFn: api.composeStatus, staleTime: 60_000, enabled: connected });

  const s: SystemInfo | undefined = sys.data;
  const info = overview.data?.info;
  const version = overview.data?.version;
  // Windows (1.9.1): sin paquete .deb, glibc ni escritorio de Linux; la vista web es WebView2.
  const win = isWindows();
  const app: Line[] = [
    [t("system.appVersion"), s?.appVersion],
    win ? [t("system.package"), t("updates.installedWin")] : [t("system.package"), s ? (s.packageVersion ?? t("system.notInstalled")) : null],
    [t("system.executable"), s?.executable],
    [win ? "WebView2" : t("system.webkit"), s?.webkitGtk],
    [t("system.configPath"), s?.configPath],
    [t("system.stateDir"), s?.stateDir],
  ];
  const host: Line[] = [
    [t("system.os"), s?.os],
    [t("system.kernel"), s?.kernel],
    [t("system.arch"), s?.arch],
    ...(win
      ? []
      : ([
          [t("system.glibc"), s?.glibc],
          [t("system.desktop"), s ? [s.desktop, s.sessionType].filter(Boolean).join(" · ") || null : null],
        ] as Line[])),
  ];
  const docker: Line[] = connected
    ? [
        [t("system.engine"), version ? `${version.version} (API ${version.apiVersion})` : null],
        [t("system.compose.label"), composeText(compose.data, t)],
        [t("system.dockerOs"), info?.operatingSystem],
        [t("system.storage"), info?.storageDriver],
        [t("system.cgroup"), info ? [info.cgroupDriver, info.cgroupVersion && `v${info.cgroupVersion}`].filter(Boolean).join(" · ") : null],
        [t("system.rootDir"), info?.dockerRootDir],
        [t("system.resources"), info?.ncpu ? `${info.ncpu} CPU · ${formatBytes(info.memTotal, i18n.language)}` : null],
        [t("system.rootless"), info ? (info.rootless ? t("diag.yes") : t("diag.no")) : null],
      ]
    : [[t("system.engine"), t("system.notConnected")]];
  const sections: Array<[string, Line[]]> = [
    [t("system.app"), app],
    [t("system.host"), host],
    [t("system.docker"), docker],
  ];

  return (
    <Group title={t("system.title")}>
      <div className="flex items-center justify-between gap-4 px-5 py-3">
        <p className="text-[12px] text-ink-3">{t("system.hint")}</p>
        <CopyButton text={systemReport(sections)} />
      </div>
      {sections.map(([title, lines]) => (
        <div key={title} className="px-5 py-3">
          <h3 className="mb-1.5 text-[12px] font-semibold text-ink-3">{title}</h3>
          <dl className="grid grid-cols-[minmax(150px,auto)_1fr] gap-x-4 gap-y-1 text-[12.5px]">
            {lines.map(([k, v]) => (
              <div key={k} className="contents">
                <dt className="text-ink-3">{k}</dt>
                <dd className="selectable min-w-0 break-all font-mono text-[12px] text-ink-2">{v ?? "—"}</dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </Group>
  );
}
