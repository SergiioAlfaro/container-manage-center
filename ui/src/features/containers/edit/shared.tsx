// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useTranslation } from "react-i18next";
import { Banner } from "@/shared/ui/feedback";

export const editKey = (id: string) => ["containerEdit", id] as const;

/** Error de una operación de edición y en qué estado quedó el contenedor. */
export function OutcomeBanner({ error, rolledBack, rollbackFailed }: { error: string; rolledBack: boolean; rollbackFailed: boolean }) {
  const { t } = useTranslation();
  return (
    <Banner tone="error">
      <p>{t("edit.failed", { message: error })}</p>
      <p className="mt-1 font-medium">{rollbackFailed ? t("edit.rollbackFailed") : rolledBack ? t("edit.rolledBack") : t("edit.nothingChanged")}</p>
    </Banner>
  );
}
