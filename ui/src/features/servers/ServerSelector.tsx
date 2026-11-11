/**
 * Selector del servidor del área principal (§7): nombre y estado de cada
 * servidor. Con un servidor remoto seleccionado, el selector se distingue a
 * simple vista del equipo local.
 */
import { useState } from "react";
import { ChevronDown, LayoutGrid, Settings2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useServers } from "@/app/server";
import { cx } from "@/shared/ui/controls";
import { Menu } from "@/shared/ui/Menu";
import { useServersNav } from "./nav";
import { statusOf } from "./remote";
import { KindIcon, StatusDot } from "./ui";

export function ServerSelector() {
  const { t } = useTranslation();
  const servers = useServers((s) => s.servers);
  const states = useServers((s) => s.states);
  const selected = useServers((s) => s.selected);
  const select = useServers((s) => s.select);
  const navigate = useApp((s) => s.navigate);
  const nav = useServersNav();
  const [open, setOpen] = useState(false);
  const current = servers.find((s) => s.id === selected);
  const remote = !!current?.remote;
  const name = current?.remote ? current.name : t("servers.local");
  const status = statusOf(states[selected] ?? current?.state);

  return (
    <div className="flex shrink-0" data-testid="server-selector">
      <Menu
        label={t("servers.selector.label", { name })}
        align="right"
        open={open}
        onOpenChange={setOpen}
        buttonClassName="rounded-md focus-visible:outline-2"
        trigger={
          <span
            className={cx(
              "inline-flex h-8 max-w-[260px] items-center gap-2 rounded-md border px-2.5 text-[12.5px]",
              remote ? "border-beacon/50 bg-beacon-soft text-beacon-strong" : "border-line bg-surface text-ink-2",
            )}
          >
            <KindIcon remote={remote} />
            <span className="truncate font-medium">{name}</span>
            <StatusDot status={status} />
            {remote && (
              <span className="rounded bg-beacon px-1 text-[10px] font-semibold uppercase leading-[15px] text-beacon-ink">{t("servers.remoteBadge")}</span>
            )}
            <ChevronDown size={13} aria-hidden className="shrink-0 opacity-70" />
          </span>
        }
        items={[
          ...servers.map((s) => ({
            id: s.id,
            label: `${s.remote ? s.name : t("servers.local")} — ${t(`servers.status.${statusOf(states[s.id] ?? s.state)}`)}${s.id === selected ? ` (${t("servers.selector.current")})` : ""}`,
            // El texto ya dice el estado: el punto es decorativo.
            icon: (
              <span aria-hidden>
                <StatusDot status={statusOf(states[s.id] ?? s.state)} />
              </span>
            ),
            onSelect: () => select(s.id),
          })),
          {
            id: "overview",
            label: t("servers.tabs.overview"),
            icon: <LayoutGrid size={14} />,
            onSelect: () => {
              nav.setTab("overview");
              navigate("servers");
            },
          },
          {
            id: "manage",
            label: t("servers.selector.manage"),
            icon: <Settings2 size={14} />,
            onSelect: () => {
              nav.setTab("servers");
              navigate("servers");
            },
          },
        ]}
      />
    </div>
  );
}
