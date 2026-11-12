import { useTranslation } from "react-i18next";
import type { EndpointSource } from "@/shared/ipc/bindings/EndpointSource";

export function SourceLabel({ source }: { source: EndpointSource }) {
  const { t } = useTranslation();
  return <>{source.kind === "context" || source.kind === "ssh" ? t(`conn.source.${source.kind}`, { name: source.name }) : t(`conn.source.${source.kind}`)}</>;
}
