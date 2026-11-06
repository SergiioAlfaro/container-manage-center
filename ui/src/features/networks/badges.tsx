import { useTranslation } from "react-i18next";
import type { NetworkSummary } from "@/shared/ipc/bindings/NetworkSummary";

export function Badge({ children, tone = "plain" }: { children: React.ReactNode; tone?: "plain" | "accent" }) {
  return <span className={`rounded px-1.5 py-0.5 text-[11px] font-medium ${tone === "accent" ? "bg-beacon/12 text-beacon-strong" : "bg-surface-sunk text-ink-3"}`}>{children}</span>;
}

/** Distintivos de una red: predefinida, interna, conectable, IPv6, Compose. */
export function NetworkBadges({ n }: { n: Pick<NetworkSummary, "predefined" | "internal" | "attachable" | "enableIpv6" | "composeProject" | "ingress"> }) {
  const { t } = useTranslation();
  return (
    <>
      {n.predefined && <Badge>{t("networks.badge.predefined")}</Badge>}
      {n.internal && <Badge>{t("networks.badge.internal")}</Badge>}
      {n.attachable && <Badge>{t("networks.badge.attachable")}</Badge>}
      {n.ingress && <Badge>ingress</Badge>}
      {n.enableIpv6 && <Badge>IPv6</Badge>}
      {n.composeProject && <Badge tone="accent">{t("networks.badge.compose", { project: n.composeProject })}</Badge>}
    </>
  );
}
