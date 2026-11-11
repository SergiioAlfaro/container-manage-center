/**
 * Menú de acciones accesible (0.9.2): botón con `aria-haspopup="menu"`, lista
 * con `role="menu"`, flechas para moverse, Escape para cerrar y clic fuera.
 * Se puede abrir desde fuera (`open`), p. ej. con el botón derecho en una fila.
 */
import { useEffect, useId, useRef, type ReactNode } from "react";
import { cx } from "./controls";

export interface MenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
  /** Descripción emergente (p. ej. por qué está desactivada). */
  title?: string;
  danger?: boolean;
}

export function Menu({
  label,
  trigger,
  items,
  open,
  onOpenChange,
  align = "right",
}: {
  /** Nombre accesible del botón. */
  label: string;
  trigger: ReactNode;
  items: MenuItem[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  align?: "left" | "right";
}) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);

  const entries = () => Array.from(list.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not([disabled])") ?? []);

  useEffect(() => {
    if (!open) return;
    entries()[0]?.focus();
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) onOpenChange(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open, onOpenChange]);

  const close = (refocus = true) => {
    onOpenChange(false);
    if (refocus) button.current?.focus();
  };

  return (
    <div ref={root} className="relative inline-block">
      <button
        ref={button}
        type="button"
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => onOpenChange(!open)}
        className={cx("grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink", open && "bg-surface-sunk text-ink")}
      >
        {trigger}
      </button>
      {open && (
        <ul
          ref={list}
          id={id}
          role="menu"
          aria-label={label}
          onKeyDown={(e) => {
            const all = entries();
            const i = all.indexOf(document.activeElement as HTMLButtonElement);
            if (e.key === "ArrowDown") {
              e.preventDefault();
              all[(i + 1) % all.length]?.focus();
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              all[(i - 1 + all.length) % all.length]?.focus();
            } else if (e.key === "Home") {
              e.preventDefault();
              all[0]?.focus();
            } else if (e.key === "End") {
              e.preventDefault();
              all[all.length - 1]?.focus();
            } else if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              close();
            } else if (e.key === "Tab") {
              close(false);
            }
          }}
          className={cx("panel-enter absolute top-8 z-40 min-w-[250px] rounded-lg border border-line bg-surface py-1 text-left shadow-xl", align === "right" ? "right-0" : "left-0")}
        >
          {items.map((it) => (
            <li key={it.id} role="none">
              <button
                type="button"
                role="menuitem"
                disabled={it.disabled}
                title={it.title}
                onClick={() => {
                  close();
                  it.onSelect();
                }}
                className={cx(
                  "flex w-full items-center gap-2 px-3 py-1.5 text-[13px] focus:bg-surface-2 focus:outline-none enabled:hover:bg-surface-2 disabled:opacity-50",
                  it.danger ? "text-err" : "text-ink",
                )}
              >
                {it.icon && <span className="shrink-0 text-ink-3">{it.icon}</span>}
                {it.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
