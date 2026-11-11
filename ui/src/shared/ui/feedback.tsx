import { useEffect, useId, useRef, type ReactNode } from "react";
import { AlertTriangle, Info, LoaderCircle, X } from "lucide-react";
import { cx } from "./controls";

export function Spinner({ size = 16, className }: { size?: number; className?: string }) {
  return <LoaderCircle size={size} className={cx("animate-spin", className)} aria-hidden />;
}

export function Banner({
  tone,
  children,
  onDismiss,
  dismissLabel,
}: {
  tone: "error" | "warn" | "info";
  children: ReactNode;
  onDismiss?: () => void;
  dismissLabel?: string;
}) {
  const Icon = tone === "info" ? Info : AlertTriangle;
  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={cx(
        "flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 text-[13px]",
        tone === "error" && "border-err/30 bg-err/8 text-ink",
        tone === "warn" && "border-warn/30 bg-warn/10 text-ink",
        tone === "info" && "border-line bg-surface-2 text-ink-2",
      )}
    >
      <Icon size={16} className={cx("mt-0.5 shrink-0", tone === "error" ? "text-err" : tone === "warn" ? "text-warn" : "text-ink-3")} />
      <div className="selectable min-w-0 flex-1 [overflow-wrap:anywhere]">{children}</div>
      {onDismiss && (
        <button type="button" onClick={onDismiss} aria-label={dismissLabel} className="text-ink-3 hover:text-ink">
          <X size={15} />
        </button>
      )}
    </div>
  );
}

export function Checkbox({ checked, onChange, label, hint, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; hint?: string; disabled?: boolean }) {
  const id = useId();
  return (
    <div className="flex gap-2.5">
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-[3px] h-4 w-4 shrink-0 accent-[var(--beacon)]"
      />
      <label htmlFor={id} className="text-[13px] leading-snug">
        {label}
        {hint && <span className="mt-0.5 block text-[12px] text-ink-3">{hint}</span>}
      </label>
    </div>
  );
}

/**
 * Diálogo modal: Escape cierra, el foco entra en el diálogo al abrir y vuelve
 * al elemento anterior al cerrar; Tab no sale del diálogo.
 */
export function Dialog({ title, children, onClose, footer, wide = false }: { title: string; children: ReactNode; onClose: () => void; footer: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const focusables = () => Array.from(el?.querySelectorAll<HTMLElement>("button, input, select, textarea, [tabindex]:not([tabindex='-1'])") ?? []).filter((x) => !x.hasAttribute("disabled"));
    focusables()[0]?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "Tab") {
        const f = focusables();
        if (f.length === 0) return;
        const first = f[0]!;
        const last = f[f.length - 1]!;
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      previous?.focus?.();
    };
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-[rgb(10_16_24/0.45)] p-6" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby={titleId} className={cx("panel-enter flex max-h-[calc(100vh-48px)] w-full flex-col rounded-xl border border-line bg-surface shadow-2xl", wide ? "max-w-[620px]" : "max-w-[460px]")}>
        <h2 id={titleId} className="border-b border-line px-5 py-3.5 text-[15px] font-semibold [overflow-wrap:anywhere]">
          {title}
        </h2>
        <div className="min-h-0 space-y-3 overflow-y-auto px-5 py-4 text-[13.5px]">{children}</div>
        <div className="flex justify-end gap-2 border-t border-line px-5 py-3">{footer}</div>
      </div>
    </div>
  );
}
