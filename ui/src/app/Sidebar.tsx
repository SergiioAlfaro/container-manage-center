// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import logo from "@/assets/logo.svg";
import { cx } from "@/shared/ui/controls";
import { SECTIONS, type SectionDef, type SectionId } from "./sections";

interface Props {
  active: SectionId;
  collapsed: boolean;
  onNavigate: (id: SectionId) => void;
  onToggle: () => void;
}

const GROUP_ORDER: Array<SectionDef["group"]> = ["top", "resources", "orchestration", "tools", "monitoring"];

export function Sidebar({ active, collapsed, onNavigate, onToggle }: Props) {
  const { t } = useTranslation();

  const item = (s: SectionDef) => {
    const Icon = s.icon;
    const isActive = s.id === active;
    const label = t(`sections.${s.id}.title`);
    return (
      <li key={s.id}>
        <button
          type="button"
          onClick={() => onNavigate(s.id)}
          aria-current={isActive ? "page" : undefined}
          title={collapsed ? label : undefined}
          className={cx(
            "group relative flex h-9 w-full items-center gap-3 rounded-md text-[13.5px] transition-colors duration-150 active:bg-hull-3",
            collapsed ? "justify-center px-0" : "px-3",
            isActive ? "bg-hull-3 text-white" : "text-hull-ink hover:bg-hull-2 hover:text-white",
          )}
        >
          {isActive && <span aria-hidden className="absolute left-0 top-2 bottom-2 w-[3px] rounded-r bg-beacon-nav" />}
          <Icon size={18} strokeWidth={1.75} className={cx("shrink-0", isActive ? "text-beacon-nav" : "text-hull-ink-dim group-hover:text-hull-ink")} />
          {!collapsed && <span className="truncate">{label}</span>}
        </button>
      </li>
    );
  };

  return (
    <nav
      aria-label={t("nav.label")}
      className={cx(
        "flex h-full shrink-0 flex-col bg-hull text-hull-ink transition-[width] duration-200",
        collapsed ? "w-[64px]" : "w-[236px]",
      )}
    >
      <div className={cx("flex h-14 items-center gap-2.5", collapsed ? "justify-center" : "px-4")}>
        <img src={logo} alt="" className="h-8 w-8 shrink-0" />
        {!collapsed && (
          <div className="min-w-0 leading-tight">
            <div className="truncate text-[14px] font-semibold text-white">CMC</div>
            <div className="truncate text-[12px] text-hull-ink-dim" data-testid="brand-name">
              Container Manage Center
            </div>
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-2.5 pb-3">
        {GROUP_ORDER.map((g) => {
          const items = SECTIONS.filter((s) => s.group === g);
          return (
            <div key={g} className={g === "top" ? "mt-2" : "mt-5"}>
              {g !== "top" &&
                (collapsed ? (
                  <div aria-hidden className="mx-3 mb-2 h-px bg-hull-2" />
                ) : (
                  <div className="mb-1.5 px-3 text-[11.5px] font-medium text-hull-ink-dim">{t(`nav.groups.${g}`)}</div>
                ))}
              <ul className="space-y-0.5">{items.map(item)}</ul>
            </div>
          );
        })}
      </div>

      <div className="border-t border-hull-2 px-2.5 py-2.5">
        <ul className="space-y-0.5">{SECTIONS.filter((s) => s.group === "bottom").map(item)}</ul>
        <button
          type="button"
          onClick={onToggle}
          aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
          title={`${collapsed ? t("nav.expand") : t("nav.collapse")} (Ctrl+B)`}
          className={cx(
            "mt-1 flex h-9 w-full items-center gap-3 rounded-md text-[13px] text-hull-ink-dim hover:bg-hull-2 hover:text-white",
            collapsed ? "justify-center" : "px-3",
          )}
        >
          {collapsed ? <PanelLeftOpen size={18} strokeWidth={1.75} /> : <PanelLeftClose size={18} strokeWidth={1.75} />}
          {!collapsed && <span>{t("nav.collapse")}</span>}
        </button>
      </div>
    </nav>
  );
}
