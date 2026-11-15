// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, BookOpen, ChevronRight, PackageX, RotateCcw } from "lucide-react";
import { DEFAULT_CONFIG, useApp } from "@/app/store";
import { LOCAL_SERVER, ServerScope } from "@/app/server";
import type { Language } from "@/shared/ipc/bindings/Language";
import type { Theme } from "@/shared/ipc/bindings/Theme";
import { LANGUAGE_NAMES, LANGUAGES } from "@/shared/i18n";
import { Button, Segmented, Switch } from "@/shared/ui/controls";
import { Dialog } from "@/shared/ui/feedback";
import { numberFormat } from "@/shared/format";
import { HelpView } from "@/features/help/HelpView";
import { ComponentsSettings } from "./ComponentsSettings";
import { DockerSettings } from "./DockerSettings";
import { LegalSettings } from "./LegalSettings";
import { SystemSettings } from "./SystemSettings";
import { UpdatesSettings } from "./UpdatesSettings";
import { Group, Row } from "./layout";

/** Muestra del color en el selector (el valor de `--beacon` en el tema claro). */

export const MIN_RECORDS = 50;
export const MAX_RECORDS = 10_000;

function MaxRecordsInput({ value, onCommit }: { value: number; onCommit: (n: number) => void }) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const commit = () => {
    const n = Number.parseInt(draft, 10);
    if (Number.isNaN(n)) return setDraft(String(value));
    const clamped = Math.min(MAX_RECORDS, Math.max(MIN_RECORDS, n));
    setDraft(String(clamped));
    if (clamped !== value) onCommit(clamped);
  };
  return (
    <input
      id="max-records"
      type="number"
      inputMode="numeric"
      min={MIN_RECORDS}
      max={MAX_RECORDS}
      step={50}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      className="h-8 w-24 rounded-md border border-line bg-surface px-2 text-right text-[13px] tabular-nums outline-none hover:border-line-strong focus:border-beacon"
    />
  );
}

/** Páginas propias dentro de Configuración (0.9.1). */
export type SettingsPage = "main" | "components" | "help";

function SubPage({ title, onBack, wide, children }: { title: string; onBack: () => void; wide?: boolean; children: React.ReactNode }) {
  const { t } = useTranslation();
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => heading.current?.focus(), []);
  return (
    <div className={wide ? "mx-auto max-w-[1100px] px-8 py-6" : "mx-auto max-w-[760px] px-8 py-6"}>
      <div className="mb-5 flex items-center gap-3">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft size={14} aria-hidden /> {t("settings.back")}
        </Button>
        <h2 ref={heading} tabIndex={-1} className="text-[16px] font-semibold text-ink outline-none">
          {title}
        </h2>
      </div>
      {children}
    </div>
  );
}

function OpenRow({ label, hint, icon, onOpen, action }: { label: string; hint: string; icon: React.ReactNode; onOpen: () => void; action: string }) {
  return (
    <Row label={label} hint={hint}>
      <Button onClick={onOpen}>
        {icon} {action} <ChevronRight size={14} aria-hidden />
      </Button>
    </Row>
  );
}

/**
 * La configuración es de esta aplicación y de este equipo (conexión local con
 * Docker, información del sistema, componentes instalados): siempre trabaja con
 * el equipo local, aunque en el selector esté elegido un servidor remoto.
 */
export function SettingsView() {
  return (
    <ServerScope server={LOCAL_SERVER}>
      <SettingsContent />
    </ServerScope>
  );
}

function SettingsContent() {
  const { t, i18n } = useTranslation();
  const { config, updateConfig } = useApp();
  const [resetting, setResetting] = useState(false);
  const [page, setPage] = useState<SettingsPage>("main");
  const nf = numberFormat(i18n.language);

  if (page === "components") {
    return (
      <SubPage title={t("components.title")} onBack={() => setPage("main")}>
        <ComponentsSettings />
      </SubPage>
    );
  }
  if (page === "help") {
    return (
      <SubPage title={t("help.title")} onBack={() => setPage("main")} wide>
        <HelpView />
      </SubPage>
    );
  }

  return (
    <div className="mx-auto max-w-[760px] px-8 py-8">
      <Group title={t("settings.appearance")}>
        <Row label={t("settings.theme")}>
          <Segmented<Theme>
            label={t("settings.theme")}
            value={config.ui.theme}
            onChange={(theme) => void updateConfig({ ui: { theme } })}
            options={(["system", "light", "dark"] as Theme[]).map((v) => ({ value: v, label: t(`settings.themeOptions.${v}`) }))}
          />
        </Row>
        <Row label={t("settings.language")} htmlFor="ui-language">
          {/* Con ocho idiomas, una lista desplegable (0.9.2). Cada idioma con su propio nombre. */}
          <select
            id="ui-language"
            value={config.ui.language}
            onChange={(e) => void updateConfig({ ui: { language: e.target.value as Language } })}
            className="h-8 min-w-[180px] rounded-md border border-line bg-surface px-2 text-[13px] text-ink outline-none hover:border-line-strong"
          >
            {LANGUAGES.map((v) => (
              <option key={v} value={v} lang={v}>
                {LANGUAGE_NAMES[v]}
              </option>
            ))}
          </select>
        </Row>
      </Group>

      <Group title={t("settings.activityPanel")}>
        <Row label={t("settings.openOnError")} hint={t("settings.openOnErrorHint")}>
          <Switch
            label={t("settings.openOnError")}
            checked={config.activity.openOnError}
            onChange={(openOnError) => void updateConfig({ activity: { openOnError } })}
          />
        </Row>
        <Row
          label={t("settings.maxRecords")}
          hint={t("settings.maxRecordsHint", { min: MIN_RECORDS, max: nf.format(MAX_RECORDS) })}
          htmlFor="max-records"
        >
          <MaxRecordsInput value={config.activity.maxRecords} onCommit={(maxRecords) => void updateConfig({ activity: { maxRecords } })} />
        </Row>
        <Row label={t("settings.keepHistory")} hint={t("settings.keepHistoryHint")}>
          <Switch
            label={t("settings.keepHistory")}
            checked={config.activity.keepHistory}
            onChange={(keepHistory) => void updateConfig({ activity: { keepHistory } })}
          />
        </Row>
      </Group>

      <DockerSettings />

      <UpdatesSettings />

      <SystemSettings />

      <LegalSettings />

      <Group title={t("settings.maintenance")}>
        <OpenRow
          label={t("components.title")}
          hint={t("components.rowHint")}
          icon={<PackageX size={14} aria-hidden />}
          action={t("settings.open")}
          onOpen={() => setPage("components")}
        />
        <OpenRow label={t("help.title")} hint={t("help.rowHint")} icon={<BookOpen size={14} aria-hidden />} action={t("settings.open")} onOpen={() => setPage("help")} />
      </Group>

      <Group title={t("settings.shortcuts")}>
        <Row label={t("settings.shortcutSidebar")}>
          <kbd className="rounded border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-[12px] text-ink-2">Ctrl+B</kbd>
        </Row>
        <Row label={t("settings.shortcutActivity")}>
          <kbd className="rounded border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-[12px] text-ink-2">{t("activity.shortcut")}</kbd>
        </Row>
      </Group>

      <Group title={t("settings.reset")}>
        <Row label={t("settings.resetLabel")} hint={t("settings.resetHint")}>
          <Button onClick={() => setResetting(true)}>
            <RotateCcw size={14} /> {t("settings.resetButton")}
          </Button>
        </Row>
      </Group>
      {resetting && (
        <Dialog
          target={false}
          title={t("settings.resetConfirmTitle")}
          onClose={() => setResetting(false)}
          footer={
            <>
              <Button onClick={() => setResetting(false)}>{t("common.cancel")}</Button>
              <Button
                variant="primary"
                onClick={() => {
                  setResetting(false);
                  void updateConfig({ ui: DEFAULT_CONFIG.ui, activity: DEFAULT_CONFIG.activity, updates: DEFAULT_CONFIG.updates });
                }}
              >
                {t("settings.resetButton")}
              </Button>
            </>
          }
        >
          <p className="text-[13px] text-ink-2">{t("settings.resetConfirmBody")}</p>
        </Dialog>
      )}
    </div>
  );
}
