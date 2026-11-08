import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import { DEFAULT_CONFIG, useApp } from "@/app/store";
import type { Language } from "@/shared/ipc/bindings/Language";
import type { Theme } from "@/shared/ipc/bindings/Theme";
import { LANGUAGES } from "@/shared/i18n";
import { Button, Segmented, Switch } from "@/shared/ui/controls";
import { Dialog } from "@/shared/ui/feedback";
import { numberFormat } from "@/shared/format";
import { DockerSettings } from "./DockerSettings";
import { SystemSettings } from "./SystemSettings";
import { UpdatesSettings } from "./UpdatesSettings";
import { Group, Row } from "./layout";

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

export function SettingsView() {
  const { t, i18n } = useTranslation();
  const { config, updateConfig } = useApp();
  const [resetting, setResetting] = useState(false);
  const nf = numberFormat(i18n.language);

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
