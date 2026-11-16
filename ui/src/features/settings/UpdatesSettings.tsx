// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ExternalLink, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { dateTimeFormat, formatDate, formatTimestamp } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { UpdateCheck } from "@/shared/ipc/bindings/UpdateCheck";
import { Button, CopyButton, Switch } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { Group, Row } from "./layout";
import { isWindows } from "@/shared/platform";

/** Resultado de una comprobación (manual o al iniciar). */
export function UpdateResult({ check }: { check: UpdateCheck }) {
  const { t, i18n } = useTranslation();
  const [openError, setOpenError] = useState<string | null>(null);
  const published = check.published ? new Date(check.published) : null;
  // Una fecha sin hora ("2026-12-01") se muestra sin hora: convertirla a la hora
  // local inventaría una ("01:00:00").
  const dayOnly = !!check.published && /^\d{4}-\d{2}-\d{2}$/.test(check.published);
  const publishedText =
    published && !Number.isNaN(published.getTime())
      ? dayOnly
        ? dateTimeFormat(i18n.language, { day: "2-digit", month: "2-digit", year: "numeric", timeZone: "UTC" }).format(published)
        : formatDate(published, i18n.language)
      : null;
  return (
    <div className="space-y-2" data-testid="update-result">
      {check.newer ? (
        <Banner tone="info">
          <strong className="text-ink">{t("updates.newer", { latest: check.latest, current: check.current })}</strong>
          {publishedText && <> · {t("updates.published", { date: publishedText })}</>}
          {check.url && (
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <Button
                size="sm"
                onClick={() => {
                  setOpenError(null);
                  api.openReleasePage(check.url!).catch((e: { message: string }) => setOpenError(e.message));
                }}
              >
                <ExternalLink size={13} /> {t("updates.openPage")}
              </Button>
              <CopyButton text={check.url} compact />
              <code className="selectable truncate font-mono text-[11.5px] text-ink-3">{check.url}</code>
            </div>
          )}
          {check.notes && (
            <details className="mt-2">
              <summary className="cursor-pointer text-[12.5px] text-ink-2">{t("updates.notes")}</summary>
              <pre className="selectable mt-1 max-h-48 overflow-auto whitespace-pre-wrap font-sans text-[12.5px] text-ink-2">{check.notes}</pre>
            </details>
          )}
        </Banner>
      ) : (
        <Banner tone="info">{t("updates.upToDate", { current: check.current, latest: check.latest })}</Banner>
      )}
      {openError && <Banner tone="error">{openError}</Banner>}
      <p className="text-[11.5px] text-ink-3">
        {t("updates.checkedAt", { url: check.checkedUrl, time: formatTimestamp(new Date(check.checkedAt), i18n.language, { date: true }) })}
      </p>
    </div>
  );
}

export function UpdatesSettings() {
  const { t } = useTranslation();
  const { config, updateConfig } = useApp();
  const saved = config.updates.source;
  const [draft, setDraft] = useState(saved);
  useEffect(() => setDraft(saved), [saved]);
  const sys = useQuery({ queryKey: ["systemInfo"], queryFn: api.systemInfo, staleTime: Infinity });
  const last = useQuery({ queryKey: ["updatesLast"], queryFn: api.updatesLast, staleTime: 0 });

  const check = useMutation({
    mutationFn: (source: string) => api.updatesCheck(source),
    // Una fuente que funciona se guarda.
    onSuccess: (_r, source) => {
      if (source.trim() !== saved) void updateConfig({ updates: { source: source.trim() } });
    },
  });
  const result = check.data ?? (check.isIdle ? (last.data?.check ?? null) : null);
  const lastError = check.isIdle && last.data?.error ? last.data.error : null;
  const installed = isWindows()
    ? t("updates.installedWin")
    : sys.data?.packageVersion
      ? t("updates.installedPackage", { version: sys.data.packageVersion })
      : t("updates.installedLocal");

  return (
    <Group title={t("updates.title")}>
      <Row label={t("updates.installed")} hint={sys.data ? installed : undefined}>
        <span className="selectable tabular-nums text-[13px] text-ink-2">{sys.data?.appVersion ?? "—"}</span>
      </Row>
      <div className="px-5 py-3.5">
        <label htmlFor="updates-source" className="text-[13.5px] text-ink">
          {t("updates.source")}
        </label>
        <p className="mt-0.5 text-[12px] text-ink-3">{t("updates.sourceHint")}</p>
        <form
          className="mt-2 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            check.mutate(draft);
          }}
        >
          <input
            id="updates-source"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            spellCheck={false}
            placeholder={t("updates.sourcePlaceholder")}
            className="h-8 min-w-0 flex-1 rounded-md border border-line bg-surface px-2 font-mono text-[12.5px] outline-none placeholder:font-sans placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
          <Button type="submit" disabled={check.isPending || draft.trim() === ""}>
            {check.isPending ? <Spinner size={14} /> : <RefreshCw size={14} />}
            {check.isPending ? t("updates.checking") : t("updates.checkNow")}
          </Button>
        </form>
        <div className="mt-3 space-y-2" aria-live="polite">
          {check.error && <Banner tone="error">{(check.error as { message: string }).message}</Banner>}
          {lastError && <Banner tone="warn">{t("updates.lastFailed", { message: lastError })}</Banner>}
          {result && <UpdateResult check={result} />}
        </div>
      </div>
      <Row label={t("updates.onStart")} hint={saved ? t("updates.onStartHint") : t("updates.onStartNoSource")}>
        <Switch
          label={t("updates.onStart")}
          checked={config.updates.checkOnStart}
          disabled={!saved}
          onChange={(checkOnStart) => void updateConfig({ updates: { checkOnStart } })}
        />
      </Row>
      <p className="px-5 py-3 text-[12px] text-ink-3">{t(isWindows() ? "updates.howToWin" : "updates.howTo")}</p>
    </Group>
  );
}
