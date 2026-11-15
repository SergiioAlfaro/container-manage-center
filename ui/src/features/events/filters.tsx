// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cx } from "@/shared/ui/controls";

/** Botones para elegir varios valores (vacío = todos). */
export function Chips<T extends string>({ items, selected, onChange, label, render }: { items: readonly T[]; selected: T[]; onChange: (v: T[]) => void; label: string; render: (v: T) => string }) {
  const { t } = useTranslation();
  return (
    <div role="group" aria-label={label} className="flex flex-wrap items-center gap-1.5">
      <button type="button" aria-pressed={selected.length === 0} onClick={() => onChange([])} className={cx("h-7 rounded-md border px-2 text-[12px]", selected.length === 0 ? "border-beacon bg-beacon/10" : "border-line text-ink-2 hover:border-line-strong")}>
        {t("events.allTypes")}
      </button>
      {items.map((it) => {
        const on = selected.includes(it);
        return (
          <button
            key={it}
            type="button"
            aria-pressed={on}
            onClick={() => onChange(on ? selected.filter((x) => x !== it) : [...selected, it])}
            className={cx("h-7 rounded-md border px-2 text-[12px]", on ? "border-beacon bg-beacon/10" : "border-line text-ink-2 hover:border-line-strong")}
          >
            {render(it)}
          </button>
        );
      })}
    </div>
  );
}

export function SearchBox({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  return (
    <label className="relative max-w-[360px] min-w-[300px] flex-1">
      <span className="sr-only">{placeholder}</span>
      <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
      />
    </label>
  );
}

export function DateField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
      {label}
      <input type="datetime-local" value={value} onChange={(e) => onChange(e.target.value)} className="h-8 rounded-md border border-line bg-surface px-2 font-mono text-[12px] text-ink outline-none hover:border-line-strong focus:border-beacon" />
    </label>
  );
}
