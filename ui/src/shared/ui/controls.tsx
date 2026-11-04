import { useEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/ipc/client";

export function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type BtnProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "ghost" | "quiet" | "primary" | "danger"; size?: "sm" | "md" };

export function Button({ variant = "quiet", size = "md", className, ...rest }: BtnProps) {
  return (
    <button
      type="button"
      className={cx(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors disabled:opacity-50 disabled:pointer-events-none",
        size === "sm" ? "h-7 px-2 text-[12px]" : "h-8 px-3 text-[13px]",
        variant === "primary" && "bg-beacon text-beacon-ink hover:bg-beacon-strong",
        variant === "danger" && "bg-err text-white hover:brightness-110",
        variant === "quiet" && "border border-line bg-surface text-ink-2 hover:text-ink hover:border-line-strong",
        variant === "ghost" && "text-ink-2 hover:text-ink hover:bg-surface-sunk",
        className,
      )}
      {...rest}
    />
  );
}

/** Control segmentado accesible (grupo de radio). */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: Array<{ value: T; label: ReactNode }>;
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex rounded-lg border border-line bg-surface-sunk p-0.5">
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(o.value)}
            className={cx(
              "h-7 rounded-md px-3 text-[13px] transition-colors",
              active ? "bg-surface text-ink shadow-[0_1px_2px_rgb(0_0_0/0.08)]" : "text-ink-2 hover:text-ink",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={cx(
        "relative h-5 w-9 shrink-0 rounded-full transition-colors",
        checked ? "bg-beacon" : "bg-line-strong",
      )}
    >
      <span
        className={cx(
          "absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform",
          checked && "translate-x-4",
        )}
      />
    </button>
  );
}

/** Copia texto al portapapeles e informa del resultado durante un instante. */
export function CopyButton({ text, compact = false }: { text: string; compact?: boolean }) {
  const { t } = useTranslation();
  const [state, setState] = useState<"idle" | "ok" | "err">("idle");
  const timer = useRef<number>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const onClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await api.copyText(text);
      setState("ok");
    } catch {
      setState("err");
    }
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setState("idle"), 1600);
  };

  const label = state === "ok" ? t("activity.copied") : state === "err" ? t("activity.copyFailed") : t("activity.copy");
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      className={cx(
        "inline-flex items-center gap-1 rounded px-1.5 text-[11px] transition-colors",
        compact ? "h-6" : "h-6 border border-line bg-surface",
        state === "ok" ? "text-ok" : state === "err" ? "text-err" : "text-ink-3 hover:text-ink",
      )}
    >
      {state === "ok" ? <Check size={13} /> : <Copy size={13} />}
      {!compact && <span>{label}</span>}
    </button>
  );
}
