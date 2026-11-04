import type { ReactNode } from "react";
import { Unplug } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { Button } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { useEngine } from "./store";

/** Muestra el contenido solo con conexión; si no, explica por qué y dónde verlo. */
export function RequireConnection({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const state = useEngine((s) => s.state);
  const navigate = useApp((s) => s.navigate);

  if (state === null || state.state === "connecting") {
    return (
      <div className="flex items-center justify-center gap-2 py-24 text-[13px] text-ink-3">
        <Spinner /> {t("conn.connecting")}
      </div>
    );
  }
  if (state.state === "disconnected") {
    return (
      <div className="mx-auto mt-16 max-w-[460px] px-6 text-center">
        <div className="mx-auto grid h-11 w-11 place-items-center rounded-lg bg-err/10 text-err">
          <Unplug size={21} strokeWidth={1.75} />
        </div>
        <h2 className="mt-4 text-[15px] font-semibold">{t("conn.notConnectedTitle")}</h2>
        <p className="mt-1.5 text-[13.5px] text-ink-2">{t("conn.notConnectedBody")}</p>
        <Button variant="primary" className="mt-5" onClick={() => navigate("dashboard")}>
          {t("conn.openDashboard")}
        </Button>
      </div>
    );
  }
  return <>{children}</>;
}
