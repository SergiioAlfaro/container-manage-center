// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useApi } from "@/app/server";
import { CopyButton } from "@/shared/ui/controls";
import { Group } from "./layout";

/** Datos legales fijos (1.7.0, D-111). No se traducen: son nombres, el
 *  identificador SPDX y el aviso de copyright tal como figuran en el paquete. */
export const LEGAL = {
  name: "Container Manage Center",
  short: "CMC",
  spdx: "GPL-3.0-or-later",
  copyright: "Copyright © 2026 Sergio Alfaro Lorite",
  licenseUrl: "https://www.gnu.org/licenses/gpl-3.0.html",
  docDir: "/usr/share/doc/container-manage-center/",
  notices: "/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md",
} as const;

/** Apartado «Legal» de Configuración: nombre, licencia, copyright y avisos de
 *  terceros. Sin enlaces que abran programas: las rutas y la URL se copian. */
export function LegalSettings() {
  const api = useApi();
  const { t } = useTranslation();
  const sys = useQuery({ queryKey: ["systemInfo"], queryFn: api.systemInfo, staleTime: Infinity });
  const rows: Array<[string, string]> = [
    [t("legal.name"), `${LEGAL.name} (${LEGAL.short})`],
    [t("legal.version"), sys.data?.appVersion ?? "—"],
    [t("legal.license"), LEGAL.spdx],
    [t("legal.copyright"), LEGAL.copyright],
  ];
  return (
    <Group title={t("legal.title")}>
      <div className="px-5 py-3">
        <dl className="grid grid-cols-[minmax(150px,auto)_1fr] gap-x-4 gap-y-1 text-[12.5px]" data-testid="legal-facts">
          {rows.map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-ink-3">{k}</dt>
              <dd className="selectable min-w-0 text-ink-2">{v}</dd>
            </div>
          ))}
        </dl>
      </div>
      <div className="space-y-2 px-5 py-3 text-[12.5px] text-ink-2">
        <p className="selectable">{t("legal.statement")}</p>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-ink-3">{t("legal.licenseText")}</span>
          <code className="selectable font-mono text-[12px]">{LEGAL.licenseUrl}</code>
          <CopyButton text={LEGAL.licenseUrl} />
        </div>
      </div>
      <div className="space-y-2 px-5 py-3 text-[12.5px] text-ink-2">
        <p>{t("legal.thirdParty")}</p>
        <div className="flex flex-wrap items-center gap-2">
          <code className="selectable font-mono text-[12px] break-all">{LEGAL.notices}</code>
          <CopyButton text={LEGAL.notices} />
        </div>
        <p className="text-ink-3">{t("legal.source")}</p>
        <p className="text-ink-3">{t("legal.independence")}</p>
      </div>
    </Group>
  );
}
