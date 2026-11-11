import { useEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Check, Copy, LoaderCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/ipc/client";

export function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type BtnProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "ghost" | "quiet" | "primary" | "danger"; size?: "sm" | "md"; loading?: boolean };

/**
 * Botón con estados visibles (0.9.1): reposo, encima, pulsado, foco con
 * teclado (anillo global de `:focus-visible`), desactivado y ocupado. Un botón
 * desactivado sigue mostrando su descripción emergente (`title`), que suele
 * explicar por qué no se puede usar.
 */
export function Button({ variant = "quiet", size = "md", loading = false, className, disabled, children, ...rest }: BtnProps) {
  return (
    <button
      type="button"
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cx(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-[color,background-color,border-color,filter,box-shadow] duration-150 disabled:cursor-not-allowed disabled:opacity-50",
        size === "sm" ? "h-7 px-2 text-[12px]" : "h-8 px-3 text-[13px]",
        variant === "primary" && "bg-beacon text-beacon-ink enabled:hover:bg-beacon-strong enabled:active:brightness-95",
        variant === "danger" && "bg-err text-white enabled:hover:brightness-110 enabled:active:brightness-95",
        variant === "quiet" && "border border-line bg-surface text-ink-2 enabled:hover:border-line-strong enabled:hover:text-ink enabled:active:bg-surface-sunk",
        variant === "ghost" && "text-ink-2 enabled:hover:bg-surface-sunk enabled:hover:text-ink enabled:active:bg-line",
        className,
      )}
      {...rest}
    >
      {loading && <LoaderCircle size={size === "sm" ? 12 : 14} className="animate-spin" aria-hidden />}
      {children}
    </button>
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
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex rounded-lg border border-line bg-surface-sunk p-0.5"
      // Patrón de grupo de opciones (0.9.2): las flechas eligen la anterior o la siguiente.
      onKeyDown={(e) => {
        const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
        if (step === 0) return;
        e.preventDefault();
        const i = options.findIndex((o) => o.value === value);
        const next = (i + step + options.length) % options.length;
        onChange(options[next]!.value);
        (e.currentTarget.querySelectorAll<HTMLButtonElement>("[role=radio]")[next])?.focus();
      }}
    >
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
              "h-7 rounded-md px-3 text-[13px] transition-colors duration-150",
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

export function Switch({ checked, onChange, label, disabled = false }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx(
        "relative h-5 w-9 shrink-0 rounded-full transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50",
        checked ? "bg-beacon enabled:hover:bg-beacon-strong" : "bg-line-strong enabled:hover:bg-ink-3",
      )}
    >
      <span
        className={cx(
          "absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform duration-150",
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
