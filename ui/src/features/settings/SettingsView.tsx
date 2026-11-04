import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import type { Language } from "@/shared/ipc/bindings/Language";
import type { Theme } from "@/shared/ipc/bindings/Theme";
import { LANGUAGES } from "@/shared/i18n";
import { Segmented, Switch } from "@/shared/ui/controls";

export const MIN_RECORDS = 50;
export const MAX_RECORDS = 10_000;

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mt-8 first:mt-0">
      <h2 className="mb-2 text-[13px] font-semibold text-ink-2">{title}</h2>
      <div className="divide-y divide-line rounded-xl border border-line bg-surface">{children}</div>
    </section>
  );
}

function Row({
  label,
  hint,
  children,
  htmlFor,
  stacked = false,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  htmlFor?: string;
  /** Valor debajo de la etiqueta, para textos largos como rutas. */
  stacked?: boolean;
}) {
  if (stacked) {
    return (
      <div className="px-5 py-3.5">
        <div className="text-[13.5px] text-ink">{label}</div>
        <div className="mt-1 min-w-0 break-all">{children}</div>
      </div>
    );
  }
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-3.5">
      <div className="min-w-0">
        <label htmlFor={htmlFor} className="text-[13.5px] text-ink">
          {label}
        </label>
        {hint && <p className="mt-0.5 text-[12px] text-ink-3">{hint}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

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

export function SettingsView() {
  const { t, i18n } = useTranslation();
  const { config, info, updateConfig } = useApp();
  const nf = new Intl.NumberFormat(i18n.language === "es" ? "es-ES" : "en-US");

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
        <Row label={t("settings.language")}>
          <Segmented<Language>
            label={t("settings.language")}
            value={config.ui.language}
            onChange={(language) => void updateConfig({ ui: { language } })}
            options={LANGUAGES.map((v) => ({ value: v, label: t(`settings.languageNames.${v}`) }))}
          />
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
      </Group>

      <Group title={t("settings.shortcuts")}>
        <Row label={t("settings.shortcutSidebar")}>
          <kbd className="rounded border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-[12px] text-ink-2">Ctrl+B</kbd>
        </Row>
        <Row label={t("settings.shortcutActivity")}>
          <kbd className="rounded border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-[12px] text-ink-2">{t("activity.shortcut")}</kbd>
        </Row>
      </Group>

      <Group title={t("settings.about")}>
        <Row label={t("settings.version")}>
          <span className="selectable tabular-nums text-[13px] text-ink-2">{info?.version ?? t("common.unknown")}</span>
        </Row>
        <Row label={t("settings.configPath")} stacked>
          <code className="selectable font-mono text-[12px] text-ink-2">{info?.configPath ?? t("common.unknown")}</code>
        </Row>
      </Group>
    </div>
  );
}
